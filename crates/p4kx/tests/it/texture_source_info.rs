// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn cli_preserves_source_words_and_legacy_fields_for_archive_and_disk() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (disk, archive) = fixture(folder.path())?;
    let mut values = Vec::new();
    for entry in [false, true] {
        let mut command = p4kx_testkit::p4kx()?;
        command
            .args(["--json", "texture", "info"])
            .arg(if entry { &archive } else { &disk });
        if entry {
            command.arg("Data/source.dds");
        }
        let output = command.output()?;
        assert!(output.status.success(), "synthetic source info succeeds");
        let value: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(
            value.get("width"),
            Some(&json!(8)),
            "legacy width remains compatible"
        );
        assert_eq!(
            value.get("height"),
            Some(&json!(4)),
            "legacy height remains compatible"
        );
        assert_eq!(
            value.get("format"),
            Some(&json!("BC7_UNORM_SRGB")),
            "legacy label remains compatible"
        );
        assert_eq!(
            value.pointer("/source_metadata/raw/legacy/reserved1/0"),
            Some(&json!(u32::MAX)),
            "unknown authored DWORD is preserved"
        );
        assert_eq!(
            value.pointer("/source_metadata/raw/dx10/misc_flags2"),
            Some(&json!(0xface_0007_u32)),
            "authored DX10 extra flags are preserved"
        );
        verify_header_origin(&value, entry, "transfer~1effective", "srgb")?;
        verify_header_origin(&value, entry, "alpha_association", "reserved")?;
        values.push(
            value
                .get("source_metadata")
                .ok_or("additive source metadata")?
                .clone(),
        );
    }
    assert_eq!(
        values.first(),
        values.get(1),
        "the same shared DTO serves disk and Archive"
    );
    Ok(())
}

fn verify_header_origin(value: &Value, entry: bool, pointer: &str, expected: &str) -> TestResult {
    let path = pointer.replace("~1", "/");
    assert_eq!(
        value
            .pointer(&format!("/source_metadata/{path}"))
            .and_then(Value::as_str),
        Some(expected),
        "{path}"
    );
    let field = value
        .pointer(&format!("/field_origins/~1source_metadata~1{pointer}"))
        .ok_or("derived DDS header field origin")?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&if entry { json!(false) } else { Value::Null }),
        "actual Archive proof is distinct from external file input"
    );
    Ok(())
}

fn fixture(root: &Path) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let mut bytes = SplitDds::new(TestTextureFormat::BC7_SRGB, 8, 4)
        .mips(1)
        .header_bytes();
    bytes
        .get_mut(32..36)
        .ok_or("reserved DWORD")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    bytes
        .get_mut(144..148)
        .ok_or("miscFlags2 DWORD")?
        .copy_from_slice(&0xface_0007_u32.to_le_bytes());
    let disk = root.join("source.dds");
    fs::write(&disk, &bytes)?;
    let archive = root.join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/source.dds", bytes))
        .write_to(&archive)?;
    Ok((disk, archive))
}
