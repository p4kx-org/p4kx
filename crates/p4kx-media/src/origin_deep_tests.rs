// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::PathBuf;
use std::thread;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::checkpoint::Checkpoint;
use crate::origin_read_policy::OriginReadPolicy;
use crate::origin_verify::{self, OriginAvailability};
use crate::{
    Capability, ExportOptions, Index, MediaError, MediaKind, MediaWriter, Projection, Provider,
    ProviderRegistry, SourceContext, Status, Subresource,
};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

const FIXTURE_STACK_BYTES: usize = 32 << 20;

struct NestedProvider {
    arrays: u32,
}

impl Provider for NestedProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Model,
            "synthetic.deep-record",
            "1.0.0",
            "retained source",
        )
    }

    fn source(
        &self,
        _source: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let mut value = serde_json::Value::Null;
        for _ in 0..self.arrays {
            value = serde_json::Value::Array(vec![value]);
        }
        writer.record(
            &Subresource::new("deep-record", Vec::new()),
            Status::Complete,
            &value,
            &[],
        )?;
        Ok(Projection::Written)
    }
}

struct Fixture {
    folder: tempfile::TempDir,
    output: PathBuf,
    index: Index,
}

#[test]
fn actual_deep_records_reuse_on_the_default_stack_without_the_serde_128_depth_gate() -> TestResult {
    for depth in [128, 192, crate::limits::MAX_VALUE_DEPTH] {
        let fixture = thread::Builder::new()
            .stack_size(FIXTURE_STACK_BYTES)
            .spawn(move || fixture(depth))?
            .join()
            .map_err(|_| io::Error::other("deep fixture export thread panicked"))??;
        verify_on_current_stack(&fixture)?;
    }
    Ok(())
}

fn fixture(depth: u32) -> Result<Fixture, Box<dyn std::error::Error + Send + Sync>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("plain.bin", b"data"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let options = ExportOptions {
        limits: crate::Limits {
            value_depth: depth,
            ..crate::Limits::default()
        },
        ..ExportOptions::default()
    };
    let registry = ProviderRegistry::try_new(vec![Box::new(NestedProvider { arrays: depth - 1 })])?;
    let output = folder.path().join("media");
    let crate::ExportOutcome::Published { index, .. } =
        crate::export_with(&archive, &output, &options, &registry)?
    else {
        return Err("old Finite depth boundary remains admitted".into());
    };
    let over = ProviderRegistry::try_new(vec![Box::new(NestedProvider { arrays: depth })])?;
    let destination = folder.path().join("over-original-depth");
    let error = crate::export_with(&archive, &destination, &options, &over)
        .err()
        .ok_or("one extra array must exceed the old Finite depth boundary")?;
    assert!(error.to_string().contains("value_depth"));
    assert!(!destination.join("media-index.json").exists());
    Ok(Fixture {
        folder,
        output,
        index: *index,
    })
}

fn verify_on_current_stack(fixture: &Fixture) -> TestResult {
    let archive = Archive::open(fixture.folder.path().join("Data.p4k"))?;
    let cancel = CancelToken::new();
    let watch = ArchiveWatch::new(&archive);
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel: &cancel,
        watch: &watch,
        protection: &protection,
    };
    let receipts = fixture
        .index
        .field_origins
        .as_ref()
        .ok_or("current complete origin group")?;
    assert_eq!(
        origin_verify::generation(
            &fixture.output.join(&fixture.index.artifact_base),
            &fixture.index,
            checkpoint,
            OriginReadPolicy::from_receipts(receipts),
        )?,
        OriginAvailability::Current,
        "the actual serialized output verifies on the ordinary test thread stack",
    );
    Ok(())
}
