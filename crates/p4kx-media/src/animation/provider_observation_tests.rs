// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use p4kx_extraction::Exclusion;
use p4kx_testkit::{TestAnimation, TestEntry, TestPositionFormat, TestTimes, TestTrack};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::artifact::StreamObserver;
use crate::checkpoint::Checkpoint;
use crate::required_input_fault_support::{self as support, Inventory, TestResult};
use crate::{
    Artifact, ChunkReference, ExportOptions, ExportOutcome, Limits, MediaError, MediaKind,
    MediaRecord, MediaWriter, OffsetBasis, Provider, SourceContext, error, hashing,
};

use super::{AnimationProvider, project_with};

const POSITIONS: [(u16, [f32; 3]); 3] = [
    (0, [0.0; 3]),
    (9, [1.5, -0.25, 0.75]),
    (30, [-0.5, 0.5, 0.0]),
];

#[derive(Default)]
struct Observation {
    records: RefCell<Vec<u8>>,
    chunks: RefCell<Vec<Artifact>>,
}

impl StreamObserver for Observation {
    fn write(&self, relative: &str, bytes: &[u8]) -> io::Result<()> {
        if relative == "animation.jsonl" {
            self.records.borrow_mut().extend_from_slice(bytes);
        }
        Ok(())
    }

    fn typed_chunk(&self, artifact: &Artifact) -> io::Result<bool> {
        let mut chunks = self.chunks.borrow_mut();
        let created = !chunks.iter().any(|seen| seen.path == artifact.path);
        chunks.push(artifact.clone());
        Ok(created)
    }
}

#[test]
fn observed_animation_matches_published_source_rows_and_key_chunks() -> TestResult {
    let caf = TestAnimation::new("synthetic.caf", 30)
        .track(
            TestTrack::new(17)
                .positions(&POSITIONS)
                .position_format(TestPositionFormat::Float)
                .times(TestTimes::Shorts),
        )
        .caf();
    support::with_fixture(
        [TestEntry::stored("synthetic.caf", caf)],
        Inventory::Intact,
        &Exclusion::default(),
        |context, _unused, checkpoint, folder| verify(context, checkpoint, folder),
    )
}

fn verify(context: &SourceContext<'_>, checkpoint: Checkpoint<'_>, folder: &Path) -> TestResult {
    let archive = context.directory.index.archive;
    let observation = Observation::default();
    let observation_root = folder.join("observed-animation");
    observe(context, checkpoint, &observation_root, &observation)?;
    assert!(
        !observation_root.exists(),
        "the observed source path stages no output"
    );

    let output = folder.join("normal-animation");
    let options = ExportOptions {
        kinds: vec![MediaKind::Animation],
        ..ExportOptions::default()
    };
    let ExportOutcome::Published { index, .. } = crate::export(archive, &output, &options)? else {
        return Err("the synthetic CAF must publish normally".into());
    };
    let generation = output.join(&index.artifact_base);
    let [kind] = index.kinds.as_slice() else {
        return Err("one actual animation provider".into());
    };
    let records = observation.records.borrow();
    assert_eq!(
        *records,
        fs::read(generation.join(&kind.artifact.path))?,
        "the observed route preserves every original source row and typed reference"
    );
    let original_payload = fs::read(context.payload)?;
    verify_keys(
        &records,
        &observation.chunks.borrow(),
        &generation,
        &original_payload,
    )
}

fn observe(
    context: &SourceContext<'_>,
    checkpoint: Checkpoint<'_>,
    root: &Path,
    observation: &Observation,
) -> TestResult {
    let archive = context.directory.index.archive;
    let mut writer = MediaWriter::observed(
        root,
        vec![AnimationProvider.capability()],
        Limits::default(),
        checkpoint,
        false,
        observation,
    )?;
    writer
        .origins
        .observe_root(archive, archive.path().metadata()?.len())?;
    let original = writer.origins.input(archive, context.entry)?;
    writer.begin_with_input(0, context.source.clone(), original)?;
    let raw = context.source.raw.as_ref().ok_or("original raw receipt")?;
    let payload = root.join(&raw.path);
    let observed = SourceContext {
        payload: &payload,
        ..*context
    };
    let projected = project_with(&observed, &mut writer, |context, writer| {
        writer.checkpoint()?;
        let reader = archive
            .open_entry_verified(context.entry)
            .map_err(|cause| MediaError::Archive(Box::new(cause)))?;
        let length = context.source.size;
        let mut bytes = Vec::new();
        reader
            .take(length.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|cause| error::io(archive.path(), cause))?;
        if u64::try_from(bytes.len()).ok() != Some(length) {
            return Err(MediaError::Contract(
                "synthetic verified animation extent differs".into(),
            ));
        }
        writer.checkpoint()?;
        Ok(bytes)
    })?;
    writer.end_source(projected)?;
    writer.finish()?;
    Ok(())
}

fn verify_keys(
    records: &[u8],
    chunks: &[Artifact],
    generation: &Path,
    original_payload: &[u8],
) -> TestResult {
    let rows = std::str::from_utf8(records)?
        .lines()
        .map(serde_json::from_str::<MediaRecord>)
        .collect::<Result<Vec<_>, _>>()?;
    let controller = rows
        .iter()
        .find(|row| row.subresource.namespace == "animation-controller")
        .ok_or("source-backed controller row")?;
    assert_eq!(controller.subresource.indices, [0, 0]);
    assert_eq!(
        chunks.len(),
        2,
        "only one frame and one position chunk are emitted"
    );
    let frames = POSITIONS
        .iter()
        .flat_map(|(frame, _)| frame.to_le_bytes())
        .collect::<Vec<_>>();
    let positions = POSITIONS
        .iter()
        .flat_map(|(_, values)| values.iter().flat_map(|value| value.to_le_bytes()))
        .collect::<Vec<_>>();
    for (field, original) in [("frames", frames), ("values", positions)] {
        verify_key_chunk(
            &controller.data,
            field,
            &original,
            chunks,
            generation,
            original_payload,
        )?;
    }
    Ok(())
}

fn verify_key_chunk(
    controller: &Value,
    field: &str,
    expected: &[u8],
    chunks: &[Artifact],
    generation: &Path,
    original_payload: &[u8],
) -> TestResult {
    let reference: ChunkReference = serde_json::from_value(
        controller
            .pointer(&format!("/payload/translation/samples/{field}"))
            .ok_or("one complete typed key reference")?
            .clone(),
    )?;
    let artifact = chunks
        .iter()
        .find(|chunk| chunk.path == reference.artifact.path)
        .ok_or("the actual observed key bytes")?;
    assert_eq!(
        artifact.size,
        u64::try_from(expected.len())?,
        "{field} byte count"
    );
    assert_eq!(
        artifact.digest, reference.artifact.digest,
        "{field} digest receipt"
    );
    assert_eq!(
        artifact.digest.hex,
        hashing::hex(Sha256::digest(expected).into()),
        "{field} digest matches the authored values"
    );
    assert_eq!(
        fs::read(generation.join(&artifact.path))?.as_slice(),
        expected,
        "published {field} bytes"
    );
    let extent = reference.layout.source_range.ok_or("key source range")?;
    assert!(matches!(extent.basis, OffsetBasis::Entry));
    let range = usize::try_from(extent.start)?..usize::try_from(extent.end_exclusive)?;
    assert_eq!(
        original_payload.get(range),
        Some(expected),
        "{field} resolves to its exact uncompressed source words"
    );
    Ok(())
}
