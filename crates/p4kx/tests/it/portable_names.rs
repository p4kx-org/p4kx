// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx extract` and `p4kx dcb export` write only file names valid on Linux and Windows.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

type TestResult = Result<(), Box<dyn Error>>;

fn database() -> Vec<u8> {
    let item = |guid: u128, name: &str, file: &str| {
        TestRecord::new(guid, "Item", name, file).value("count", TestValue::Int32(7))
    };
    DataForgeBuilder::new()
        .structure(TestStruct::new("Item").field("count", TestType::Int32))
        .record(item(1, "Item.a?b", "records/shared.xml"))
        .record(item(2, "Item.fine", "records/shared.xml"))
        .record(item(3, "Item.pipe", "records/a|b.xml"))
        .record(item(4, "Item.literal", "records/a%7Cb.xml"))
        .build()
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database()).encrypted())
        .entry(TestEntry::stored("Data\\a<b.txt", b"bad".to_vec()))
        .entry(TestEntry::stored("Data\\ok.txt", b"fine".to_vec()))
        .write_to(&path)?;
    Ok(path)
}

fn files(root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut found = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                let relative = path.strip_prefix(root)?.display().to_string();
                found.push(relative.replace('\\', "/"));
            }
        }
    }
    found.sort();
    Ok(found)
}

#[test]
fn extract_reports_an_entry_with_a_forbidden_character_and_writes_the_others() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    let output = p4kx_testkit::p4kx()?
        .arg("extract")
        .arg(&archive)
        .args(["--filter", "Data\\"])
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(1), "partial failure: {output:?}");
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("failed Data\\a<b.txt") && stderr.contains("'<'"),
        "the Entry and the character are named: {stderr}"
    );
    assert_eq!(
        files(&out)?,
        [
            "Data/Game2.dcb",
            "Data/Game2.dcb.p4kx-origin.jsonl",
            "Data/ok.txt",
            "Data/ok.txt.p4kx-origin.jsonl",
        ],
        "the other Entries are written"
    );
    Ok(())
}

#[test]
fn dcb_export_writes_a_record_under_its_guid_and_escapes_the_record_path() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "export"])
        .arg(&archive)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    assert_eq!(
        files(&out)?,
        [
            "records/a%257Cb.xml",
            "records/a%257Cb.xml.p4kx-origin.jsonl",
            "records/a%7Cb.xml",
            "records/a%7Cb.xml.p4kx-origin.jsonl",
            "records/shared/00000000-0000-0000-0000-000000000001.xml",
            "records/shared/00000000-0000-0000-0000-000000000001.xml.p4kx-origin.jsonl",
            "records/shared/Item.fine.xml",
            "records/shared/Item.fine.xml.p4kx-origin.jsonl",
        ],
        "each Record and its companion have distinct portable paths"
    );
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("renamed records/shared (00000000-0000-0000-0000-000000000001)")
            && stderr.contains("renamed records/a|b (00000000-0000-0000-0000-000000000003)"),
        "the report names both Records: {stderr}"
    );
    Ok(())
}
