// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn source() -> Vec<u8> {
    ArchiveBuilder::new()
        .entry(TestEntry::stored("same.txt", b"one"))
        .entry(TestEntry::stored("same.txt", b"two"))
        .build()
}

fn response(archive: &Path, arguments: &[&str]) -> Result<Value, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(arguments)
        .arg(archive)
        .output()?;
    assert!(
        output.status.success(),
        "synthetic listing succeeds: {output:?}"
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[test]
fn list_metadata_belongs_to_each_duplicate_physical_record_and_preserves_fixed_bits() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("synthetic.p4k");
    let mut bytes = source();
    let record = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(at, magic)| (magic == b"PK\x01\x02").then_some(at))
        .nth(1)
        .ok_or("second directory record")?;
    for (offset, value) in [
        (4, 0x03ff_u16),
        (6, 0x02aa),
        (8, 0xf00d),
        (34, 0x1234),
        (36, 0x9669),
    ] {
        bytes
            .get_mut(record + offset..record + offset + 2)
            .ok_or("fixed source word")?
            .copy_from_slice(&value.to_le_bytes());
    }
    bytes
        .get_mut(record + 38..record + 42)
        .ok_or("external source attributes")?
        .copy_from_slice(&0xf123_4567_u32.to_le_bytes());
    std::fs::write(&path, bytes)?;
    let output = response(&path, &["list"])?;
    let entries = output["entries"].as_array().ok_or("physical Entry rows")?;
    assert_eq!(entries.len(), 2, "duplicate paths retain separate rows");
    for (owner, row) in entries.iter().enumerate() {
        assert_eq!(
            row["metadata"]["owner_entry_index"], owner,
            "physical owner"
        );
        assert_eq!(
            row["metadata"]["original_directory_ordinal"],
            owner.to_string(),
            "source ordinal"
        );
        assert_eq!(
            row["metadata"]["raw_v2_encryption_word"],
            Value::Null,
            "format absence"
        );
        assert_eq!(row["size"], 3, "existing numeric sizes retain meaning");
    }
    assert_eq!(
        entries[1]["metadata"]["zip_header"],
        json!({
            "raw_version_made_by": 0x03ff_u16, "raw_version_needed": 0x02aa_u16,
            "general_purpose_flags": 0xf00d_u16, "disk_start": 0x1234_u16,
            "internal_attributes": 0x9669_u16, "external_attributes": 0xf123_4567_u32
        }),
        "uninterpreted fixed bits match independent source words"
    );
    Ok(())
}

#[test]
fn actual_recovery_uses_null_source_ordinals_without_erasing_physical_owners() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("recovered.p4k");
    let mut bytes = source();
    bytes.truncate(bytes.len().checked_sub(22).ok_or("end record")?);
    std::fs::write(&path, bytes)?;
    let output = response(&path, &["--recover", "list"])?;
    let entries = output["entries"]
        .as_array()
        .ok_or("recovered physical rows")?;
    assert_eq!(entries.len(), 2, "surviving physical records");
    for (owner, row) in entries.iter().enumerate() {
        assert_eq!(
            row["metadata"]["owner_entry_index"], owner,
            "admitted index is known"
        );
        assert_eq!(
            row["metadata"]["original_directory_ordinal"],
            Value::Null,
            "an admitted recovery index is not an original directory position"
        );
    }
    Ok(())
}
