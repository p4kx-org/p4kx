// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx dcb export --combined` end to end, over a synthetic DataForge.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

type TestResult = Result<(), Box<dyn Error>>;

const COMPANION: &str = "Game2.xml.p4kx-origin.jsonl";

#[path = "dcb_export_combined_origins.rs"]
mod origins;

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Large"])
        .structure(
            TestStruct::new("Item")
                .field("mass", TestType::Single)
                .field("size", TestType::Enum("Size".to_owned())),
        )
        .structure(
            TestStruct::new("Ship")
                .parent("Item")
                .field("maker", TestType::Reference)
                .field("name", TestType::String),
        )
        .record(
            TestRecord::new(
                0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f,
                "Ship",
                "Ship.Avenger",
                "libs/foundry/records/ships/avenger.xml",
            )
            .value("mass", TestValue::Single(25_000.5))
            .value("size", TestValue::text("Large"))
            .value(
                "maker",
                TestValue::Reference(0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d),
            )
            .value("name", TestValue::text("Avenger & <Titan>")),
        )
        .record(
            TestRecord::new(
                0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d,
                "Item",
                "Item.Box",
                "libs/foundry/records/Items/boxes.xml",
            )
            .value("mass", TestValue::Single(1.0))
            .value("size", TestValue::text("Small")),
        )
        .record(
            TestRecord::new(
                0x0000_0001_0002_0003_0004_0000_0000_0005,
                "Item",
                "Item.Crate",
                "libs/foundry/records/items/boxes.xml",
            )
            .value("mass", TestValue::Single(2.5))
            .value("size", TestValue::Raw(vec![0xff; 4])),
        )
        .record(
            TestRecord::new(
                0x0000_0001_0002_0003_0004_0000_0000_0006,
                "Item",
                "Item.Anchor",
                "libs/foundry/records/anchors/anchor.xml",
            )
            .value("mass", TestValue::Single(900.0))
            .value("size", TestValue::text("Large")),
        )
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    write_archive_of(dir, &database())
}

fn write_archive_of(dir: &Path, database: &DataForgeBuilder) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    Ok(path)
}

fn export(archive: &Path, out: &Path, args: &[&str], code: i32) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "export", "--combined"])
        .arg(archive)
        .arg("--out")
        .arg(out)
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
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

fn listing(folder: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut names = fs::read_dir(folder)?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    names.sort();
    Ok(names)
}

#[test]
fn combined_writes_every_record_into_one_file_in_record_path_order() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    let stdout = export(&archive, &out, &[], 0)?;

    assert_eq!(
        listing(&out)?,
        ["Game2.xml", COMPANION],
        "one file named after the Entry"
    );
    let xml = fs::read_to_string(out.join("Game2.xml"))?;
    roxmltree::Document::parse(&xml)?;
    insta::assert_snapshot!("combined_file", xml);
    insta::assert_snapshot!("combined_stdout", stdout);
    Ok(())
}

#[test]
fn combined_with_a_filter_holds_only_the_selected_records() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    export(&archive, &out, &["--filter", "*/items/*"], 0)?;

    insta::assert_snapshot!(
        "combined_filtered",
        fs::read_to_string(out.join("Game2.xml"))?
    );
    Ok(())
}

#[test]
fn combined_is_byte_identical_across_runs() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let first = dir.path().join("first");
    let second = dir.path().join("second");

    export(&archive, &first, &[], 0)?;
    export(&archive, &second, &[], 0)?;

    assert_eq!(
        fs::read(first.join("Game2.xml"))?,
        fs::read(second.join("Game2.xml"))?,
        "the same bytes"
    );
    Ok(())
}

#[test]
fn combined_applies_the_conflict_policies_to_the_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    let file = out.join("Game2.xml");
    export(&archive, &out, &[], 0)?;
    let original = fs::read(&file)?;
    fs::write(&file, "changed")?;

    let skip = export(&archive, &out, &[], 0)?;
    let kept = fs::read(&file)?;
    export(&archive, &out, &["--conflict", "if-changed"], 0)?;
    let refreshed = fs::read(&file)?;
    let unchanged = export(&archive, &out, &["--conflict", "if-changed"], 0)?;
    fs::write(&file, "changed")?;
    export(&archive, &out, &["--conflict", "overwrite"], 0)?;

    assert_eq!(kept, b"changed", "skip keeps the existing file");
    assert!(
        skip.starts_with("skipped Game2.xml: already exists"),
        "skip reports the kept file: {skip}"
    );
    assert_eq!(refreshed, original, "if-changed rewrites a changed file");
    assert!(
        unchanged.starts_with("skipped Game2.xml: unchanged")
            && unchanged.contains("exported 0 Records, skipped 1"),
        "if-changed keeps an unchanged file: {unchanged}"
    );
    assert_eq!(fs::read(&file)?, original, "overwrite replaces the file");
    assert_eq!(
        listing(&out)?,
        ["Game2.xml", COMPANION],
        "no temporary file remains"
    );
    Ok(())
}

#[test]
fn combined_prints_one_line_of_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let stdout = export(&archive, &dir.path().join("out"), &["--json"], 0)?;

    let mut value: serde_json::Value = serde_json::from_str(&stdout)?;
    crate::dcb_export_count_origins::remove_count_origin(&mut value)?;
    insta::assert_snapshot!("combined_json", serde_json::to_string_pretty(&value)?);
    Ok(())
}
