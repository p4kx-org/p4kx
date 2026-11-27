// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx dcb export --format json|ndjson` end to end, over a synthetic DataForge.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

#[path = "dcb_export_ndjson_origins.rs"]
mod ndjson_origins;
#[path = "dcb_export_json_origins.rs"]
mod origins;

type TestResult = Result<(), Box<dyn Error>>;

const CART: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;
const WOODWORKS: u128 = 0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d;
const MISSING: u128 = 0x0000_0001_0002_0003_0004_0000_0000_0005;

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Large"])
        .structure(TestStruct::new("Wheel").field("size", TestType::Int32))
        .structure(TestStruct::new("Manufacturer").field("code", TestType::String))
        .structure(
            TestStruct::new("Vehicle")
                .field("maker", TestType::Reference)
                .array("wheels", TestType::Class("Wheel".to_owned()))
                .field("mass", TestType::Single)
                .field("spare", TestType::StrongPointer("Wheel".to_owned()))
                .field("trailer", TestType::StrongPointer("Wheel".to_owned()))
                .field("front", TestType::WeakPointer("Wheel".to_owned()))
                .array("tags", TestType::String)
                .field("label", TestType::Locale)
                .field("size", TestType::Enum("Size".to_owned()))
                .field("lost", TestType::Reference),
        )
        .instance(
            "spare",
            TestInstance::new("Wheel").value("size", TestValue::Int32(15)),
        )
        .record(cart())
        .record(
            TestRecord::new(
                WOODWORKS,
                "Manufacturer",
                "Manufacturer.Woodworks",
                "libs/foundry/records/makers/woodworks.xml",
            )
            .value("code", TestValue::text("WOOD")),
        )
        .record(
            TestRecord::new(
                MISSING,
                "Manufacturer",
                "Manufacturer.Elm",
                "libs/foundry/records/makers/woodworks.xml",
            )
            .value("code", TestValue::text("ELM \"oak\"")),
        )
}

fn cart() -> TestRecord {
    let wheel = |size: i32| {
        TestValue::Struct(TestInstance::new("Wheel").value("size", TestValue::Int32(size)))
    };
    TestRecord::new(
        CART,
        "Vehicle",
        "Vehicle.Cart",
        "libs/foundry/records/vehicles/cart.xml",
    )
    .value("maker", TestValue::Reference(WOODWORKS))
    .value("wheels", TestValue::Array(vec![wheel(16), wheel(17)]))
    .value("mass", TestValue::Single(f32::INFINITY))
    .value("spare", TestValue::pointer("spare"))
    .value("front", TestValue::pointer("spare"))
    .value(
        "tags",
        TestValue::Array(vec![TestValue::text("wood"), TestValue::text("old")]),
    )
    .value("label", TestValue::text("@vehicle_Cart"))
    .value("size", TestValue::Raw(vec![0xff; 4]))
    .value("lost", TestValue::Reference(0x77))
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .write_to(&path)?;
    Ok(path)
}

fn export(archive: &Path, out: &Path, args: &[&str], code: i32) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "export"])
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

/// Returns every payload file below `root` with its content, in path order.
fn tree(root: &Path) -> Result<String, Box<dyn Error>> {
    let mut files = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else if !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".p4kx-origin.jsonl"))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut listing = String::new();
    for file in files {
        let relative = file.strip_prefix(root)?.display().to_string();
        listing.push_str("== ");
        listing.push_str(&relative.replace('\\', "/"));
        listing.push('\n');
        listing.push_str(&fs::read_to_string(&file)?);
    }
    Ok(listing)
}

#[test]
fn json_writes_one_file_per_record_under_its_record_path() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    let stdout = export(&archive, &out, &["--format", "json"], 0)?;

    insta::assert_snapshot!("json_files", tree(&out)?);
    insta::assert_snapshot!("json_stdout", stdout);
    Ok(())
}

#[test]
fn ndjson_writes_one_record_per_line_in_record_path_order() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    let stdout = export(&archive, &out, &["--format", "ndjson"], 0)?;

    let listing = tree(&out)?;
    insta::assert_snapshot!("ndjson_file", listing);
    insta::assert_snapshot!("ndjson_stdout", stdout);
    for line in listing.lines().skip(1) {
        serde_json::from_str::<serde_json::Value>(line)?;
    }
    Ok(())
}

#[test]
fn json_and_ndjson_are_byte_identical_across_runs() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    for format in ["json", "ndjson"] {
        let first = dir.path().join(format!("{format}-first"));
        let second = dir.path().join(format!("{format}-second"));

        export(&archive, &first, &["--format", format], 0)?;
        export(&archive, &second, &["--format", format], 0)?;

        assert_eq!(tree(&first)?, tree(&second)?, "{format}: identical bytes");
    }
    Ok(())
}

#[test]
fn the_filter_selects_the_records_of_the_ndjson_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    export(
        &archive,
        &out,
        &["--format", "ndjson", "--filter", "makers"],
        0,
    )?;

    let text = fs::read_to_string(out.join("records.ndjson"))?;
    assert_eq!(text.lines().count(), 2, "the two makers: {text}");
    assert!(
        text.lines()
            .all(|line| line.contains("\"$type\":\"Manufacturer\"")),
        "{text}"
    );
    Ok(())
}

#[test]
fn the_conflict_policies_apply_to_the_ndjson_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    let file = out.join("records.ndjson");
    export(&archive, &out, &["--format", "ndjson"], 0)?;
    let original = fs::read(&file)?;
    fs::write(&file, "changed")?;

    let skip = export(&archive, &out, &["--format", "ndjson"], 0)?;
    let kept = fs::read(&file)?;
    let changed = export(
        &archive,
        &out,
        &["--format", "ndjson", "--conflict", "if-changed"],
        0,
    )?;
    let unchanged = export(
        &archive,
        &out,
        &["--format", "ndjson", "--conflict", "if-changed"],
        0,
    )?;

    assert_eq!(kept, b"changed", "skip keeps the file");
    assert!(
        skip.starts_with("skipped records.ndjson: already exists"),
        "{skip}"
    );
    assert!(changed.starts_with("exported 3 Records"), "{changed}");
    assert_eq!(fs::read(&file)?, original, "if-changed rewrites it");
    assert!(
        unchanged.starts_with("skipped records.ndjson: unchanged"),
        "{unchanged}"
    );
    Ok(())
}

#[test]
fn ndjson_prints_one_line_of_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let stdout = export(
        &archive,
        &dir.path().join("out"),
        &["--format", "ndjson", "--json"],
        0,
    )?;

    assert_eq!(stdout.lines().count(), 1, "one line: {stdout}");
    let mut value: serde_json::Value = serde_json::from_str(&stdout)?;
    crate::dcb_export_count_origins::remove_count_origin(&mut value)?;
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[test]
fn combined_with_a_json_format_exits_with_code_2() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "export"])
        .arg(&archive)
        .arg("--out")
        .arg(dir.path().join("out"))
        .args(["--format", "ndjson", "--combined"])
        .output()?;

    assert_eq!(output.status.code(), Some(2), "invalid input: {output:?}");
    assert!(
        String::from_utf8(output.stderr)?.contains("--combined writes Record XML"),
        "the reason is named"
    );
    Ok(())
}
