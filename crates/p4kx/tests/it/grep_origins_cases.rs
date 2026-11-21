// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Language;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::{TestResult, forge, write};

fn origins<'a>(report: &'a Value, path: &str) -> Result<&'a Value, Box<dyn std::error::Error>> {
    report["fieldOrigins"]
        .get(path)
        .ok_or_else(|| format!("missing origin for {path}").into())
}

fn source_uses<'a>(
    report: &'a Value,
    origin: &Value,
) -> Result<Vec<&'a Value>, Box<dyn std::error::Error>> {
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    origin["sourceUsesRefs"]
        .as_array()
        .ok_or("source references")?
        .iter()
        .map(|source_ref| {
            uses.iter()
                .find(|candidate| candidate["id"] == *source_ref)
                .ok_or_else(|| format!("unresolved source reference {source_ref}").into())
        })
        .collect()
}

fn position(
    report: &Value,
    section: &str,
    value: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    report[section]
        .as_array()
        .ok_or("result section")?
        .iter()
        .position(|row| row["text"] == value)
        .ok_or_else(|| format!("missing {section} hit {value}").into())
}

fn archive() -> ArchiveBuilder {
    let mut utf16 = vec![0xFF, 0xFE];
    utf16.extend("gladius decoded".encode_utf16().flat_map(u16::to_le_bytes));
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge()).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"key=gladius first\r\nkey=gladius second\r\n".to_vec(),
        ))
        .entry(
            TestEntry::stored(
                Language::German.global_ini_path(),
                b"key=gladius german\r\n".to_vec(),
            )
            .encrypted(),
        )
        .entry(TestEntry::stored(
            "Data\\duplicate.txt",
            b"gladius clear".to_vec(),
        ))
        .entry(TestEntry::stored("Data\\duplicate.txt", b"gladius protected".to_vec()).encrypted())
        .entry(TestEntry::stored("Data\\converted.txt", utf16))
}

#[test]
fn json_binds_hits_to_their_source_payloads_and_metadata() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write(dir.path(), &archive())?;
    let output = p4kx_testkit::p4kx()?
        .args(["--json", "grep"])
        .arg(&archive)
        .arg("gladius")
        .output()?;
    assert_eq!(output.status.code(), Some(0), "grep output: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;

    verify_records(&report)?;
    verify_localization(&report)?;
    verify_entries(&report)
}

fn verify_records(report: &Value) -> TestResult {
    let record = origins(report, "/records/0/type")?;
    assert_eq!(
        record["containsEncryptedSource"], true,
        "original DataForge is encrypted"
    );
    let record_uses = source_uses(report, record)?;
    assert!(
        record_uses.iter().any(|source| source
            .pointer("/location/namespace")
            .and_then(Value::as_str)
            == Some("dataforgeStructDefinition")),
        "type names include their original structure definition: {record_uses:?}"
    );
    assert_eq!(
        origins(report, "/records/0/match/1")?["containsEncryptedSource"],
        true,
        "range element has an exact companion"
    );
    Ok(())
}

fn verify_localization(report: &Value) -> TestResult {
    for (text, index, encrypted) in [
        ("gladius first", "0", false),
        ("gladius second", "1", false),
        ("gladius german", "0", true),
    ] {
        let row = position(report, "loc", text)?;
        let payload = origins(report, &format!("/loc/{row}/text"))?;
        assert_eq!(
            payload["containsEncryptedSource"], encrypted,
            "language payload {text}"
        );
        let uses = source_uses(report, payload)?;
        let [source] = uses.as_slice() else {
            return Err(format!("expected one source for {text}: {uses:?}").into());
        };
        assert_eq!(
            source.pointer("/location/indices"),
            Some(&json!([index])),
            "original line for {text}"
        );
        assert_eq!(
            origins(report, &format!("/loc/{row}/language"))?["containsEncryptedSource"],
            false,
            "language name comes from directory metadata"
        );
    }
    Ok(())
}

fn verify_entries(report: &Value) -> TestResult {
    let clear = position(report, "entries", "gladius clear")?;
    let encrypted = position(report, "entries", "gladius protected")?;
    let clear_use = origins(report, &format!("/entries/{clear}/text"))?;
    let encrypted_use = origins(report, &format!("/entries/{encrypted}/text"))?;
    assert_eq!(
        clear_use["containsEncryptedSource"], false,
        "clear duplicate payload"
    );
    assert_eq!(
        encrypted_use["containsEncryptedSource"], true,
        "encrypted duplicate payload"
    );
    assert_eq!(
        origins(report, &format!("/entries/{encrypted}/path"))?["containsEncryptedSource"],
        false,
        "same Entry has clear directory path"
    );
    assert_ne!(
        source_uses(report, clear_use)?
            .first()
            .map(|row| &row["source"]),
        source_uses(report, encrypted_use)?
            .first()
            .map(|row| &row["source"]),
        "identical paths retain distinct physical Entry IDs"
    );

    let decoded = position(report, "entries", "gladius decoded")?;
    let offset = origins(report, &format!("/entries/{decoded}/offset"))?;
    assert_eq!(
        offset["sourceSetComplete"], false,
        "decoded offset has no original byte map"
    );
    assert_eq!(
        offset["containsEncryptedSource"],
        Value::Null,
        "incomplete clear coordinates stay unknown"
    );
    assert_eq!(
        source_uses(report, offset)?
            .first()
            .and_then(|source| source.pointer("/location/type")),
        Some(&json!("opaque")),
        "actual UTF-16 source is retained without a false coordinate"
    );
    Ok(())
}
