// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::CancelToken;
use p4kx_media::{Artifact, ExportOptions, ExportOutcome};

use super::converted_artifact_support::{self as encoded, Mode};
use super::support::{self, TestResult};

#[test]
fn unknown_length_custom_outputs_have_exact_files_manifest_hashes_and_dedup_counts() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = encoded::archive(folder.path())?;
    let output = folder.path().join("media");
    let providers = encoded::registry(Mode::Repeat, CancelToken::new())?;
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &ExportOptions::default(),
        &providers,
    )?)?;
    let kind = index.kinds.first().ok_or("provider summary")?;
    let converted = kind.counts.converted_outputs;
    assert_eq!(
        (converted.enumerated, converted.written, converted.skipped),
        (2, 1, 1)
    );
    assert_eq!(
        (
            index.source_entries.enumerated,
            index.source_entries.written
        ),
        (1, 1)
    );
    let records = support::records(&output, &index, "texture.jsonl")?;
    let row = records.first().ok_or("actual record")?;
    let artifacts: Vec<Artifact> = serde_json::from_value(row.data.clone())?;
    assert_eq!(artifacts.len(), 2);
    for artifact in &artifacts {
        encoded::assert_artifact(&output, &index, artifact, encoded::PAYLOAD)?;
    }
    let first = artifacts.first().ok_or("converted artifact")?;
    encoded::assert_manifest(&output, &index, &first.path, 2)?;
    let previous = fs::read(output.join("media-index.json"))?;
    assert!(matches!(
        p4kx_media::export_with(&archive, &output, &ExportOptions::default(), &providers)?,
        ExportOutcome::Skipped { .. }
    ));
    assert_eq!(fs::read(output.join("media-index.json"))?, previous);
    fs::write(
        output.join(&index.artifact_base).join(&first.path),
        b"corrupt",
    )?;
    assert!(
        p4kx_media::export_with(&archive, &output, &ExportOptions::default(), &providers).is_err(),
        "skip validates actual encoded bytes"
    );
    Ok(())
}

#[test]
fn zero_length_encodings_are_measured_as_zero_bytes_without_invented_payload() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = encoded::archive(folder.path())?;
    let output = folder.path().join("media");
    let providers = encoded::registry(Mode::Empty, CancelToken::new())?;
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &ExportOptions::default(),
        &providers,
    )?)?;
    let records = support::records(&output, &index, "texture.jsonl")?;
    let row = records.first().ok_or("actual record")?;
    let artifacts: Vec<Artifact> = serde_json::from_value(row.data.clone())?;
    encoded::assert_artifact(
        &output,
        &index,
        artifacts.first().ok_or("empty artifact")?,
        b"",
    )?;
    let converted = index
        .kinds
        .first()
        .ok_or("summary")?
        .counts
        .converted_outputs;
    assert_eq!(
        (converted.enumerated, converted.written, converted.skipped),
        (1, 1, 0)
    );
    Ok(())
}
