// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{
    Archive, Exclusion, ExtractOptions, FolderSource, RawEntryArtifact, convert_folder, extract,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::bulk_wav_origin::field_uses;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn raw_files_bind_the_actual_entry_bytes_and_encryption() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive_path = directory.path().join("raw-proof.p4k");
    let target = directory.path().join("out");
    fs::create_dir(&target)?;
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data/secret.bin", b"payload".to_vec()).encrypted())
        .entry(TestEntry::stored("Data/empty.bin", Vec::new()))
        .write_to(&archive_path)?;
    let archive = Archive::open_with_key(&archive_path, Some(test_key()))?;
    let entries: Vec<_> = archive.entries().iter().collect();
    let report = extract(&archive, &entries, &target, &ExtractOptions::default());

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.extracted(), 2, "two installed original payloads");
    let [empty, secret] = report.raw_artifacts() else {
        return Err("two sorted original artifact reports".into());
    };
    assert_raw_origin(&target, empty, "Data/empty.bin", b"", false)?;
    assert_raw_origin(&target, secret, "Data/secret.bin", b"payload", true)
}

#[test]
fn folder_input_skips_generated_receipts_and_keeps_literal_suffixes() -> TestResult {
    const LITERAL: &str = "Data/native.p4kx-origin.jsonl";
    let directory = tempfile::tempdir()?;
    let archive_path = directory.path().join("native.p4k");
    let input = directory.path().join("extracted");
    let output = directory.path().join("converted");
    fs::create_dir(&input)?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored(LITERAL, b"ordinary input".to_vec()))
        .write_to(&archive_path)?;
    let archive = Archive::open(&archive_path)?;
    let entries: Vec<_> = archive.entries().iter().collect();
    let options = ExtractOptions::default();
    let extracted = extract(&archive, &entries, &input, &options);
    assert!(extracted.is_complete(), "{extracted:?}");
    assert!(input.join(format!("{LITERAL}.p4kx-origin.jsonl")).is_file());

    let folder = FolderSource::scan(&input, None)?;
    let selected = folder.select(None, &Exclusion::default());
    assert_eq!(selected.len(), 1, "only the native file is an input");
    let copied = convert_folder(&folder, &selected, true, &output, &options)?;
    assert!(copied.is_complete(), "{copied:?}");
    let [artifact] = copied.raw_artifacts() else {
        return Err("one converted-folder payload report".into());
    };
    assert_eq!(artifact.path, LITERAL, "the native path survives");
    assert_eq!(fs::read(output.join(LITERAL))?, b"ordinary input");
    assert_eq!(
        fs::read_dir(output.join("Data"))?.count(),
        2,
        "the copied file and its newly bound receipt"
    );
    Ok(())
}

fn assert_raw_origin(
    target: &Path,
    artifact: &RawEntryArtifact,
    path: &str,
    expected: &[u8],
    encrypted: bool,
) -> TestResult {
    assert_eq!(artifact.path, path, "reported original path");
    assert_eq!(fs::read(target.join(path))?, expected, "original bytes");
    let receipt = artifact.origin_receipt.as_ref().ok_or("actual companion")?;
    let rows = fs::read_to_string(target.join(receipt))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let digest = serde_json::to_value(&artifact.digest)?;
    assert_eq!(
        rows.first().and_then(|row| row.pointer("/binding/sha256")),
        digest.get("sha256"),
        "report and receipt bind the same output"
    );
    let payload = rows
        .iter()
        .find(|row| row.get("containsEncryptedSource").is_some())
        .ok_or("opaque payload origin")?;
    assert_eq!(
        payload.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "the payload has its immediate original encryption"
    );
    assert_eq!(
        payload.get("sourceSetComplete"),
        Some(&json!(true)),
        "the opaque payload has its complete original source"
    );
    assert!(
        field_uses(&rows, payload)?.iter().any(|row| {
            row.get("role") == Some(&json!("payload"))
                && row.pointer("/location/namespace") == Some(&json!("archiveEntryDecodedBytes"))
                && row.pointer("/location/indices")
                    == Some(&json!(["0", expected.len().to_string()]))
        }),
        "the payload keeps the exact half-open decoded range, even when empty"
    );
    Ok(())
}
