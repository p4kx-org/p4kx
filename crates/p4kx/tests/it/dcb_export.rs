// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx dcb export` end to end, over synthetic DataForge databases.

use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

type TestResult = Result<(), Box<dyn Error>>;

#[path = "dcb_export_xml_origins.rs"]
mod origins;

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Large"])
        .structure(
            TestStruct::new("Item")
                .field("mass", TestType::Single)
                .field("size", TestType::Enum("Size".to_owned()))
                .field("label", TestType::Locale),
        )
        .structure(
            TestStruct::new("Ship")
                .parent("Item")
                .field("crew", TestType::Int32)
                .field("name", TestType::String)
                .field("hidden", TestType::Bool),
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
            .value("label", TestValue::text("@vehicle_NameAEGS_Avenger"))
            .value("crew", TestValue::Int32(2))
            .value("name", TestValue::text("Avenger & <Titan>")),
        )
        .record(
            TestRecord::new(
                0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d,
                "Item",
                "Item.Box",
                "libs/foundry/records/items/boxes.xml",
            )
            .value("mass", TestValue::Single(1.0))
            .value("size", TestValue::text("Small"))
            .value("label", TestValue::text("@item_NameBox")),
        )
        .record(
            TestRecord::new(
                0x0000_0001_0002_0003_0004_0000_0000_0005,
                "Item",
                "Item.Crate",
                "libs/foundry/records/items/boxes.xml",
            )
            .value("mass", TestValue::Single(2.5))
            .value("size", TestValue::Raw(vec![0xff; 4]))
            .value("label", TestValue::text("@item_NameCrate")),
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

struct Run {
    stdout: String,
    stderr: String,
}

fn export(archive: &Path, out: &Path, args: &[&str], code: i32) -> Result<Run, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "export"])
        .arg(archive)
        .arg("--out")
        .arg(out)
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    let dir = archive
        .parent()
        .ok_or("the Archive has a folder")?
        .display()
        .to_string();
    let redact = |bytes: Vec<u8>| -> Result<String, Box<dyn Error>> {
        Ok(String::from_utf8(bytes)?
            .replace(&dir.replace('\\', "\\\\"), "[DIR]")
            .replace(&dir, "[DIR]")
            .replace("\\\\", "/")
            .replace('\\', "/"))
    };
    Ok(Run {
        stdout: redact(output.stdout)?,
        stderr: redact(output.stderr)?,
    })
}

/// Returns every XML file below `root` with its content, in path order.
fn tree(root: &Path) -> Result<String, Box<dyn Error>> {
    let mut files = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|extension| extension == "xml") {
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
fn dcb_export_writes_one_file_per_record_under_its_record_path() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    let run = export(&archive, &out, &["--format", "xml"], 0)?;

    insta::assert_snapshot!("export_files", tree(&out)?);
    insta::assert_snapshot!("export_stdout", run.stdout);
    assert!(run.stderr.is_empty(), "no warnings: {}", run.stderr);
    Ok(())
}

#[test]
fn dcb_export_is_byte_identical_across_runs() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    export(&archive, &dir.path().join("first"), &[], 0)?;
    export(&archive, &dir.path().join("second"), &[], 0)?;

    assert_eq!(
        tree(&dir.path().join("first"))?,
        tree(&dir.path().join("second"))?,
        "the same files with the same bytes"
    );
    Ok(())
}

#[test]
fn dcb_export_filter_selects_records_by_record_path() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let substring = dir.path().join("substring");
    let glob = dir.path().join("glob");

    export(&archive, &substring, &["--filter", "ITEMS\\boxes"], 0)?;
    export(&archive, &glob, &["--filter", "*/ships/*"], 0)?;

    insta::assert_snapshot!("filter_substring", tree(&substring)?);
    insta::assert_snapshot!("filter_glob", tree(&glob)?);
    Ok(())
}

#[test]
fn dcb_export_with_a_filter_that_matches_nothing_exits_with_code_2() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let run = export(
        &archive,
        &dir.path().join("out"),
        &["--filter", "nothing"],
        2,
    )?;

    assert_eq!(
        run.stderr, "error: no Record matches nothing\n",
        "the Filter is named"
    );
    Ok(())
}

#[test]
fn dcb_export_applies_the_conflict_policies() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    export(&archive, &out, &[], 0)?;
    let avenger = out.join("libs/foundry/records/ships/avenger.xml");
    let original = fs::read(&avenger)?;
    fs::write(&avenger, "changed")?;

    let skip = export(&archive, &out, &["--conflict", "skip"], 0)?;
    let kept = fs::read(&avenger)?;
    let if_changed = export(&archive, &out, &["--conflict", "if-changed"], 0)?;
    let refreshed = fs::read(&avenger)?;
    let unchanged = export(&archive, &out, &["--conflict", "if-changed"], 0)?;
    fs::write(&avenger, "changed")?;
    export(&archive, &out, &["--conflict", "overwrite"], 0)?;

    assert_eq!(kept, b"changed", "skip keeps the existing file");
    assert!(
        skip.stdout
            .contains("skipped libs/foundry/records/ships/avenger.xml: already exists"),
        "skip reports the kept file: {}",
        skip.stdout
    );
    assert_eq!(refreshed, original, "if-changed rewrites a changed file");
    assert!(
        if_changed
            .stdout
            .starts_with("skipped libs/foundry/records/items/boxes/Item.Box.xml: unchanged"),
        "if-changed keeps the unchanged files: {}",
        if_changed.stdout
    );
    assert!(
        unchanged.stdout.contains("exported 0 Records, skipped 3"),
        "nothing changed: {}",
        unchanged.stdout
    );
    assert_eq!(fs::read(&avenger)?, original, "overwrite replaces the file");
    Ok(())
}

#[test]
fn dcb_export_prints_one_line_of_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let run = export(&archive, &dir.path().join("out"), &["--json"], 0)?;

    assert_eq!(run.stdout.lines().count(), 1, "one line: {}", run.stdout);
    let mut value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    let receipts = value
        .as_object_mut()
        .and_then(|summary| summary.remove("origin_receipts"))
        .ok_or("per-Record origin receipts")?;
    assert_eq!(
        receipts.as_array().ok_or("receipt paths")?.len(),
        3,
        "every XML payload has a companion"
    );
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn nested_database() -> DataForgeBuilder {
    let wheel = |size: i32| {
        TestValue::Struct(TestInstance::new("Wheel").value("size", TestValue::Int32(size)))
    };
    DataForgeBuilder::new()
        .structure(TestStruct::new("Wheel").field("size", TestType::Int32))
        .structure(TestStruct::new("Manufacturer").field("code", TestType::String))
        .structure(
            TestStruct::new("Vehicle")
                .field("maker", TestType::Reference)
                .array("wheels", TestType::Class("Wheel".to_owned()))
                .field("spare", TestType::StrongPointer("Wheel".to_owned()))
                .field("trailer", TestType::StrongPointer("Wheel".to_owned()))
                .field("front", TestType::WeakPointer("Wheel".to_owned()))
                .array("tags", TestType::String)
                .array("parts", TestType::Reference),
        )
        .instance(
            "spare",
            TestInstance::new("Wheel").value("size", TestValue::Int32(15)),
        )
        .record(
            TestRecord::new(
                0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f,
                "Vehicle",
                "Vehicle.Cart",
                "libs/foundry/records/vehicles/cart.xml",
            )
            .value(
                "maker",
                TestValue::Reference(0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d),
            )
            .value("wheels", TestValue::Array(vec![wheel(16), wheel(17)]))
            .value("spare", TestValue::pointer("spare"))
            .value("front", TestValue::pointer("spare"))
            .value(
                "tags",
                TestValue::Array(vec![TestValue::text("wood"), TestValue::text("old")]),
            )
            .value(
                "parts",
                TestValue::Array(vec![
                    TestValue::Reference(0x0000_0001_0002_0003_0004_0000_0000_0005),
                    TestValue::Null,
                ]),
            ),
        )
        .record(
            TestRecord::new(
                0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d,
                "Manufacturer",
                "Manufacturer.Woodworks",
                "libs/foundry/records/makers/woodworks.xml",
            )
            .value("code", TestValue::text("WOOD")),
        )
}

#[test]
fn dcb_export_writes_nested_values_as_elements_in_declaration_order() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive_of(dir.path(), &nested_database())?;
    let first = dir.path().join("first");
    let second = dir.path().join("second");

    let run = export(&archive, &first, &[], 0)?;
    export(&archive, &second, &[], 0)?;

    insta::assert_snapshot!("export_nested", tree(&first)?);
    insta::assert_snapshot!("export_nested_stdout", run.stdout);
    assert_eq!(tree(&first)?, tree(&second)?, "byte-identical across runs");
    Ok(())
}
