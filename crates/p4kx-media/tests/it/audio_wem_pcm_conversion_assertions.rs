// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{ByteOrder, Checksum, Wem};
use p4kx_media::{Artifact, Index, MediaRecord, Status};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::super::origins::{self, Origins};
use super::{LOOSE_ROW, Original, RATE, TestResult, indices, pcm_bytes};

pub(super) fn check_mapping(
    (ordinal, row): (usize, &MediaRecord),
    namespace: &str,
    original: &Original,
    origins: &Origins,
    owner: usize,
) -> TestResult<Artifact> {
    assert_eq!(
        row.subresource.namespace, namespace,
        "one actual conversion namespace"
    );
    let indices = indices(owner);
    assert_eq!(row.subresource.indices, indices, "actual occurrence route");
    assert_eq!(row.status, Status::Complete, "PCM conversion is complete");
    assert_eq!(
        row.data.pointer("/payload/requested_format"),
        Some(&json!("wav")),
        "requested format"
    );
    assert_eq!(
        row.data.pointer("/payload/source_codec"),
        Some(&json!("pcm")),
        "actual source codec"
    );
    assert_eq!(
        row.data.pointer("/payload/outcome/type"),
        Some(&json!("written")),
        "successful write"
    );
    let payload = row
        .data
        .pointer("/payload/outcome/payload")
        .ok_or("written mapping")?;
    check_mapping_data(payload, original);
    let actual = origins.for_field(
        ordinal,
        owner,
        "/data/payload/outcome/payload/artifact/digest/hex",
    )?;
    check_pcm_uses(&actual, original, indices);
    Ok(serde_json::from_value(payload["artifact"].clone())?)
}

fn check_mapping_data(payload: &Value, original: &Original) {
    assert_eq!(payload["actual_format"], "wav", "verified output container");
    assert_eq!(payload["actual_codec"], "pcm", "verified output codec");
    assert_eq!(payload["channels"], 1, "converted channels");
    assert_eq!(payload["sample_rate"], RATE, "converted sample rate");
    assert_eq!(payload["sample_count"], "4", "converted whole frame count");
    assert_eq!(
        payload["loop_points"],
        json!({"start":"1","end":"3","frames":"2"}),
        "converted loop"
    );
    assert_eq!(
        payload["source_range"],
        json!({
            "basis":{"type":"entry"}, "start":original.base + original.samples.start,
            "end_exclusive":original.base + original.samples.end
        }),
        "half-open range of selected source frames in the original Entry"
    );
}

pub(super) fn check_wav(directory: &Path, artifact: &Artifact) -> TestResult {
    assert!(
        Path::new(&artifact.path)
            .extension()
            .is_some_and(|suffix| suffix.eq_ignore_ascii_case("wav")),
        "artifact has the actual WAV extension"
    );
    let bytes = fs::read(directory.join(&artifact.path))?;
    assert_eq!(
        artifact.size,
        u64::try_from(bytes.len())?,
        "actual placed size"
    );
    assert_eq!(
        artifact.digest.hex,
        Checksum::Sha256(Sha256::digest(&bytes).into()).hex(),
        "actual placed digest"
    );
    let wav = Wem::parse(&bytes)?;
    assert_eq!(
        wav.byte_order(),
        ByteOrder::Little,
        "canonical RIFF byte order"
    );
    assert_eq!(
        wav.data(),
        pcm_bytes(),
        "all actual decoded samples match exactly"
    );
    let loop_points = wav.loop_points().ok_or("original loop survives")?;
    assert_eq!(
        (loop_points.start, loop_points.end),
        (1, 3),
        "actual WAV loop is end-exclusive"
    );
    Ok(())
}

pub(super) fn check_manifest(
    directory: &Path,
    index: &Index,
    wav: &Artifact,
    original: &[Original; 2],
    origins: &Origins,
) -> TestResult {
    let manifest = fs::read_to_string(directory.join(&index.artifact_manifest.path))?;
    let artifacts = manifest
        .lines()
        .map(serde_json::from_str::<Artifact>)
        .collect::<Result<Vec<_>, _>>()?;
    let selected: Vec<_> = artifacts
        .iter()
        .enumerate()
        .filter(|(_, artifact)| artifact.path == wav.path)
        .collect();
    let [loose, bank] = selected.as_slice() else {
        return Err("both actual occurrences have manifest rows".into());
    };
    for (owner, ((ordinal, artifact), original)) in
        [*loose, *bank].into_iter().zip(original).enumerate()
    {
        assert_eq!(
            artifact.digest.hex, wav.digest.hex,
            "actual manifest digest"
        );
        let actual = origins.for_stream("artifacts", ordinal, owner, "/digest/hex")?;
        check_pcm_uses(&actual, original, indices(owner));
        let size = origins.for_stream("artifacts", ordinal, owner, "/size")?;
        assert!(
            !origins::contains(&size, "originalSamples"),
            "size is independent of sample bytes"
        );
    }
    Ok(())
}

fn check_pcm_uses(uses: &[&Value], original: &Original, prefix: &[u64]) {
    let indices: Vec<_> = prefix
        .iter()
        .copied()
        .chain([original.samples.start, original.samples.end])
        .map(|index| index.to_string())
        .collect();
    let expected = json!({
        "type":"subresource", "namespace":"wemPcmFrames", "field":"originalSamples", "indices":indices
    });
    assert!(
        uses.iter()
            .any(|used| used["location"] == expected && used["role"] == "conversion"),
        "actual selected PCM byte range is the conversion's input"
    );
    if !prefix.is_empty() {
        assert!(
            origins::contains(uses, "dataOffset"),
            "original embedded selection is retained"
        );
    }
}

pub(super) fn check_refusal(rows: &[MediaRecord], directory: &Path, original: &[u8]) -> TestResult {
    let conversion = rows
        .iter()
        .find(|row| row.subresource.namespace == LOOSE_ROW)
        .ok_or("explicit failure mapping")?;
    assert_eq!(
        conversion.status,
        Status::Partial,
        "unsupported WAV has real projection loss"
    );
    assert_eq!(
        conversion.data.pointer("/payload/source_codec"),
        Some(&json!("vorbis")),
        "actual codec"
    );
    assert_eq!(
        conversion.data.pointer("/payload/outcome"),
        Some(&json!({
            "type":"failed","payload":{"type":"unsupported-codec"}
        })),
        "typed refusal without a fabricated artifact"
    );
    let raw = conversion
        .source
        .raw
        .as_ref()
        .ok_or("retained original WEM")?;
    assert_eq!(
        fs::read(directory.join(&raw.path))?,
        original,
        "exact raw source remains available"
    );
    Ok(())
}
