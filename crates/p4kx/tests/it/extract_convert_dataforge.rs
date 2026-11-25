// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx extract --convert` over a synthetic Archive with a DataForge Entry.

use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

type TestResult = Result<(), Box<dyn Error>>;

const README_RECEIPT: &str = "Data/readme.txt.p4kx-origin.jsonl";

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Item")
                .field("mass", TestType::Single)
                .field("label", TestType::String),
        )
        .record(
            TestRecord::new(
                0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d,
                "Item",
                "Item.Box",
                "libs/foundry/records/items/boxes.xml",
            )
            .value("mass", TestValue::Single(1.0))
            .value("label", TestValue::text("Box")),
        )
        .record(
            TestRecord::new(
                0x0000_0001_0002_0003_0004_0000_0000_0005,
                "Item",
                "Item.Crate",
                "libs/foundry/records/items/boxes.xml",
            )
            .value("mass", TestValue::Single(2.5))
            .value("label", TestValue::Raw(vec![0xff; 4])),
        )
        .record(
            TestRecord::new(
                0x0000_0001_0002_0003_0004_0000_0000_0006,
                "Item",
                "Item.Anchor",
                "libs/foundry/records/anchors/anchor.xml",
            )
            .value("mass", TestValue::Single(900.0))
            .value("label", TestValue::text("Anchor")),
        )
        .build()
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database()).encrypted())
        .entry(TestEntry::stored("Data\\readme.txt", b"hello".to_vec()))
        .write_to(&path)?;
    Ok(path)
}

fn extract(archive: &Path, out: &Path, args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .arg("extract")
        .arg(archive)
        .args(["--filter", "Data\\"])
        .arg("--out")
        .arg(out)
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    assert!(output.stderr.is_empty(), "no warnings: {output:?}");
    let dir = archive
        .parent()
        .ok_or("the Archive has a folder")?
        .display()
        .to_string();
    Ok(String::from_utf8(output.stdout)?
        .replace(&dir.replace('\\', "\\\\"), "[DIR]")
        .replace(&dir, "[DIR]")
        .replace("\\\\", "/")
        .replace('\\', "/"))
}

/// Returns the payload files with their content, in path order.
fn tree(root: &Path) -> Result<String, Box<dyn Error>> {
    let checked_companion = root.join(README_RECEIPT);
    let mut files = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else if path != checked_companion {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut listing = String::new();
    for file in files {
        let relative = file.strip_prefix(root)?.display().to_string();
        writeln!(listing, "== {}", relative.replace('\\', "/"))?;
        listing.push_str(&fs::read_to_string(&file)?);
    }
    Ok(listing)
}

#[test]
fn convert_writes_the_dataforge_entry_as_record_files() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    let stdout = extract(&archive, &out, &["--convert"])?;

    check_readme_receipt(&out)?;
    insta::assert_snapshot!("convert_dataforge_files", tree(&out)?);
    insta::assert_snapshot!("convert_dataforge_stdout", stdout);
    Ok(())
}

fn check_readme_receipt(out: &Path) -> TestResult {
    let receipt = fs::read_to_string(out.join(README_RECEIPT))?;
    let rows: Vec<serde_json::Value> = receipt
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let header = rows.first().ok_or("receipt header")?;
    assert_eq!(
        header["binding"],
        serde_json::json!({
            "type": "file",
            "path": {"encoding": "utf8", "value": "readme.txt"},
            "byteLength": "5",
            "sha256": "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        }),
        "the receipt binds the original readme bytes",
    );
    let payload = rows
        .iter()
        .find(|row| {
            row.pointer("/address/type")
                .and_then(serde_json::Value::as_str)
                == Some("opaquePayload")
        })
        .ok_or("readme payload origin")?;
    assert_eq!(
        payload["sourceSetComplete"], true,
        "the raw readme source is complete"
    );
    assert_eq!(
        payload["containsEncryptedSource"], false,
        "the readme is plain Archive data"
    );
    assert_eq!(
        rows.last().ok_or("receipt end")?["type"],
        "end",
        "the receipt has a terminal row"
    );
    Ok(())
}

#[test]
fn convert_reports_the_records_as_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let stdout = extract(&archive, &dir.path().join("out"), &["--convert", "--json"])?;

    let value: serde_json::Value = serde_json::from_str(&stdout)?;
    insta::assert_snapshot!(
        "convert_dataforge_json",
        serde_json::to_string_pretty(&value)?
    );
    Ok(())
}

#[test]
fn without_convert_the_dcb_is_extracted_unchanged() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    let stdout = extract(&archive, &out, &[])?;

    assert_eq!(
        fs::read(out.join("Data/Game2.dcb"))?,
        database(),
        "the raw .dcb"
    );
    assert!(!out.join("Data/libs").exists(), "no Record files");
    assert!(
        stdout.starts_with("extracted 2, skipped 0, failed 0, cancelled 0"),
        "no Record counts: {stdout}"
    );
    Ok(())
}
