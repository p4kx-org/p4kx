// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx texture info|merge` end to end, for Entries of an Archive and for files on disk.

use std::error::Error;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

const HEADER: &str = "Data\\Textures\\hull_ddna.dds";

fn texture() -> SplitDds {
    SplitDds::new(TestTextureFormat::BC7_SRGB, 64, 32).split(4)
}

fn write_archive(folder: &Path, dds: &SplitDds) -> Result<PathBuf, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    dds.entries(HEADER)
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

fn p4kx() -> Result<Command, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?)
}

/// Checks the merged file byte by byte: the 148-byte DX10 header of the header Entry, then the
/// seven mips from 64×32 down to 1×1, each at its offset in the standard order.
fn assert_standard_layout(merged: &[u8], dds: &SplitDds) {
    let header = dds.header_bytes();
    assert_eq!(
        merged.get(..148),
        header.get(..148),
        "the DDS header comes first"
    );
    let mut offset = 148;
    for (level, size) in [128, 32, 8, 2, 1, 1, 1].into_iter().enumerate() {
        let len = size * 16;
        let level = u32::try_from(level).unwrap_or_default();
        assert_eq!(
            merged.get(offset..offset + len),
            Some(dds.mip(level).as_slice()),
            "mip {level} at offset {offset}"
        );
        offset += len;
    }
    assert_eq!(merged.len(), offset, "nothing after the smallest mip");
}

#[test]
fn info_of_an_entry_prints_format_size_mips_faces_and_parts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &texture())?;

    p4kx()?
        .args(["texture", "info"])
        .arg(&archive)
        .arg("data/textures/HULL_DDNA.dds")
        .assert()
        .code(0)
        .stdout(predicate::str::is_match(
            r"(?m)^Texture: +Data\\Textures\\hull_ddna\.dds$",
        )?)
        .stdout(predicate::str::is_match(r"(?m)^Format: +BC7_UNORM_SRGB$")?)
        .stdout(predicate::str::is_match(r"(?m)^Size: +64 x 32$")?)
        .stdout(predicate::str::is_match(r"(?m)^Depth: +1$")?)
        .stdout(predicate::str::is_match(r"(?m)^Mips: +7$")?)
        .stdout(predicate::str::is_match(r"(?m)^Array size: +1$")?)
        .stdout(predicate::str::is_match(r"(?m)^Faces: +1$")?)
        .stdout(predicate::str::is_match(r"(?m)^Parts: +4$")?)
        .stdout(predicate::str::is_match(r"(?m)^Alpha tail: +none$")?);
    Ok(())
}

#[test]
fn info_of_a_split_cubemap_reports_its_faces_and_its_alpha_tail() -> TestResult {
    let dir = tempfile::tempdir()?;
    let dds = SplitDds::new(TestTextureFormat::LegacyDxt1, 32, 32)
        .cubemap()
        .split(3)
        .alpha(TestTextureFormat::R8);
    let archive = write_archive(dir.path(), &dds)?;

    p4kx()?
        .args(["texture", "info"])
        .arg(&archive)
        .arg(HEADER)
        .assert()
        .code(0)
        .stdout(predicate::str::is_match(r"(?m)^Faces: +6$")?)
        .stdout(predicate::str::is_match(r"(?m)^Parts: +3$")?)
        .stdout(predicate::str::is_match(
            r"(?m)^Alpha tail: +R8_UNORM, 3 parts$",
        )?);
    Ok(())
}

#[test]
fn info_of_a_file_prints_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = SplitDds::new(TestTextureFormat::LegacyDxt1, 32, 32)
        .cubemap()
        .split(2)
        .alpha(TestTextureFormat::R8)
        .write_files(dir.path(), "sky_cm.dds")?;

    let output = p4kx()?
        .args(["--json", "texture", "info"])
        .arg(&header)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success");
    let mut json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let metadata = json
        .as_object_mut()
        .ok_or("info JSON object")?
        .remove("source_metadata")
        .ok_or("additive source metadata")?;
    assert!(
        metadata.is_object(),
        "the shared additive source projection is present"
    );
    check_disk_origins(&mut json)?;
    assert_eq!(
        json,
        serde_json::json!({
            "texture": header.display().to_string(),
            "format": "BC1_UNORM",
            "width": 32,
            "height": 32,
            "depth": 1,
            "mip_count": 6,
            "array_size": 1,
            "faces": 6,
            "cubemap": true,
            "parts": 2,
            "alpha_tail": {"format": "R8_UNORM", "parts": 2, "error": null},
        }),
        "the JSON shape"
    );
    Ok(())
}

fn check_disk_origins(json: &mut serde_json::Value) -> TestResult {
    let object = json.as_object_mut().ok_or("info JSON object")?;
    let fields = object.remove("field_origins").ok_or("field origin map")?;
    let width = fields.get("/width").ok_or("width origin")?;
    assert_eq!(
        width.get("containsEncryptedSource"),
        Some(&serde_json::Value::Null),
        "standalone encryption is unknown"
    );
    assert_eq!(
        width.get("sourceSetComplete"),
        Some(&serde_json::json!(true)),
        "actual disk source is bound"
    );
    assert!(
        object.remove("provenance").is_some(),
        "original file inventory"
    );
    let tail = object
        .get_mut("alpha_tail")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("alpha metadata")?;
    assert!(
        tail.remove("source_metadata").is_some(),
        "additive alpha DDS metadata"
    );
    Ok(())
}
#[test]
fn info_reports_a_broken_alpha_tail_instead_of_failing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = SplitDds::new(TestTextureFormat::LegacyDxt1, 8, 8)
        .alpha(TestTextureFormat::R8)
        .write_files(dir.path(), "glass.dds")?;
    std::fs::write(dir.path().join("glass.dds.a"), b"not a texture")?;

    p4kx()?
        .args(["texture", "info"])
        .arg(&header)
        .assert()
        .code(0)
        .stdout(predicate::str::is_match(r"(?m)^Format: +BC1_UNORM$")?)
        .stdout(predicate::str::is_match(
            r"(?m)^Alpha tail: +broken: .*glass\.dds\.a.*not a DDS texture",
        )?);
    let output = p4kx()?
        .args(["--json", "texture", "info"])
        .arg(&header)
        .output()?;
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json["alpha_tail"]["format"],
        serde_json::Value::Null,
        "no format"
    );
    assert!(
        json["alpha_tail"]["error"]
            .as_str()
            .is_some_and(|error| error.contains("not a DDS texture")),
        "the error: {json}"
    );
    Ok(())
}

#[test]
fn merge_of_an_entry_writes_the_standard_dds_layout() -> TestResult {
    let dir = tempfile::tempdir()?;
    let dds = texture();
    let archive = write_archive(dir.path(), &dds)?;
    let out = dir.path().join("hull_ddna.dds");

    p4kx()?
        .args(["texture", "merge"])
        .arg(&archive)
        .arg(HEADER)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("4 parts"));

    assert_standard_layout(&std::fs::read(&out)?, &dds);
    Ok(())
}

#[test]
fn merge_of_a_file_with_its_parts_next_to_it_writes_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let dds = texture();
    let header = dds.write_files(dir.path(), "hull_ddna.dds")?;
    let out = dir.path().join("merged.dds");

    let output = p4kx()?
        .args(["texture", "merge", "--json"])
        .arg(&header)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success");
    let merged = std::fs::read(&out)?;
    assert_standard_layout(&merged, &dds);
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json,
        serde_json::json!({
            "texture": header.display().to_string(),
            "output": out.display().to_string(),
            "parts": 4,
            "size": merged.len(),
        }),
        "the JSON shape"
    );
    Ok(())
}

#[test]
fn a_missing_part_fails_naming_the_part() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = texture().write_files(dir.path(), "hull_ddna.dds")?;
    std::fs::remove_file(dir.path().join("hull_ddna.dds.3"))?;
    let out = dir.path().join("merged.dds");

    p4kx()?
        .args(["texture", "merge"])
        .arg(&header)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("hull_ddna.dds.3 is missing"));

    assert!(!out.exists(), "nothing is written");
    Ok(())
}

#[test]
fn an_unknown_entry_exits_with_code_2() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &texture())?;

    p4kx()?
        .args(["texture", "info"])
        .arg(&archive)
        .arg("Data\\nothing.dds")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no Entry named Data\\nothing.dds"));
    Ok(())
}

#[test]
fn a_file_that_is_no_texture_fails() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("notes.dds");
    std::fs::write(&file, "plain text, not a texture")?;

    p4kx()?
        .args(["texture", "info"])
        .arg(&file)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not a DDS texture"));
    Ok(())
}

#[test]
fn unreadable_entry_data_or_an_unreadable_file_is_a_read_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let header = texture().header_bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::deflated(HEADER, header).with_payload(vec![0xFF; 64]))
        .write_to(&archive)?;
    let folder = dir.path().join("folder.dds");
    std::fs::create_dir(&folder)?;

    p4kx()?
        .args(["texture", "info"])
        .arg(&archive)
        .arg(HEADER)
        .assert()
        .code(3);
    p4kx()?
        .args(["texture", "info"])
        .arg(&folder)
        .assert()
        .code(3);
    Ok(())
}

#[test]
fn info_json_keeps_encrypted_alpha_receipts_separate_from_the_color_header() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let dds = texture().alpha(TestTextureFormat::R8);
    let tail = dds.alpha_tail().ok_or("alpha fixture")?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored(HEADER, dds.header_bytes()))
        .entry(TestEntry::stored(format!("{HEADER}.a"), tail.header_bytes()).encrypted())
        .write_to(&archive)?;
    let output = p4kx()?
        .args(["texture", "info", "--json"])
        .arg(&archive)
        .arg(HEADER)
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let info: serde_json::Value = serde_json::from_slice(&output)?;
    let fields = info.get("field_origins").ok_or("per-field evidence")?;
    for (field, expected) in [
        ("/width", false),
        ("/source_metadata/raw/dx10/misc_flags2", false),
        ("/alpha_tail/format", true),
        ("/alpha_tail/source_metadata/normalized/width", true),
    ] {
        assert_eq!(
            fields
                .get(field)
                .and_then(|origin| origin.get("containsEncryptedSource")),
            Some(&serde_json::json!(expected)),
            "encryption marker for {field}"
        );
    }
    assert_eq!(
        info.pointer("/alpha_tail/source_metadata/raw/magic_present"),
        Some(&serde_json::json!(false)),
        "alpha raw header is exported independently"
    );
    Ok(())
}
