// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx compare --schema` end to end over pairs of synthetic DataForges.

use std::error::Error;
use std::path::Path;

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType};

type TestResult = Result<(), Box<dyn Error>>;

const OLD: &str = "LIVE/Data.p4k";
const NEW: &str = "PTU/Data.p4k";

const SHIP: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0001;

fn database(size: &[&str], speed: TestType, extra: bool) -> Vec<u8> {
    let ship = TestStruct::new("Ship")
        .field("speed", speed)
        .field("size", TestType::Enum("Size".to_owned()));
    let ship = if extra {
        ship.field("armor", TestType::Bool)
    } else {
        ship
    };
    DataForgeBuilder::new()
        .enumeration("Size", size.iter().copied())
        .structure(ship)
        .record(TestRecord::new(SHIP, "Ship", "Ship.a", "libs/ships/a.xml"))
        .build()
}

fn old_database() -> Vec<u8> {
    database(&["Small", "Large"], TestType::Int32, false)
}

fn new_database() -> Vec<u8> {
    database(&["Small", "Large", "Huge"], TestType::Single, true)
}

fn write(dir: &Path, archive: &str, dataforge: Vec<u8>) -> TestResult {
    let path = dir.join(archive);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", dataforge))
        .write_to(path)?;
    Ok(())
}

fn pair(old: Vec<u8>, new: Vec<u8>) -> Result<tempfile::TempDir, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    write(dir.path(), OLD, old)?;
    write(dir.path(), NEW, new)?;
    Ok(dir)
}

#[test]
fn an_archive_without_a_dataforge_exits_with_code_5() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(dir.path(), OLD, old_database())?;
    let bare = dir.path().join(NEW);
    std::fs::create_dir_all(bare.parent().ok_or("folder")?)?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\readme.txt", b"hi".to_vec()))
        .write_to(&bare)?;

    let output = schema(dir.path(), &[])?.output()?;

    assert_eq!(output.status.code(), Some(5), "exit code: {output:?}");
    assert!(
        String::from_utf8(output.stderr)?.contains("holds no DataForge"),
        "the error names the missing DataForge"
    );
    Ok(())
}

fn schema(dir: &Path, extra: &[&str]) -> Result<Command, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .current_dir(dir)
        .args(["compare", OLD, NEW, "--schema"])
        .args(extra);
    Ok(command)
}

fn stdout(command: &mut Command, code: i32) -> Result<String, Box<dyn Error>> {
    let output = command.output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn a_changed_schema_prints_its_changes_and_exits_with_code_1() -> TestResult {
    let dir = pair(old_database(), new_database())?;

    let text = stdout(&mut schema(dir.path(), &[])?, 1)?;

    for expected in [
        "Structs:",
        "0 added, 0 removed, 1 changed",
        "Enum options:",
        "1 added, 0 removed, 0 changed",
        "mapped instances 1 -> 1",
        "+ armor: Bool",
        "~ speed: Int32 -> Single (type)",
        "+ Huge",
    ] {
        assert!(text.contains(expected), "{expected:?} in\n{text}");
    }
    Ok(())
}

#[test]
fn the_json_report_lists_the_changes() -> TestResult {
    let dir = pair(old_database(), new_database())?;

    let text = stdout(&mut schema(dir.path(), &["--json"])?, 1)?;

    let report: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(report.pointer("/format_version"), Some(&1.into()));
    assert_eq!(report.pointer("/source/old/channel"), Some(&"LIVE".into()));
    assert_eq!(report.pointer("/structs/0/name"), Some(&"Ship".into()));
    assert_eq!(report.pointer("/structs/0/records/old"), Some(&1.into()));
    assert_eq!(
        report.pointer("/structs/0/mapped_instances/new"),
        Some(&1.into()),
        "the report exposes original physical instance counts"
    );
    assert_eq!(
        report.pointer("/structs/0/properties/0/name"),
        Some(&"armor".into())
    );
    assert_eq!(
        report.pointer("/structs/0/properties/1/aspects/0"),
        Some(&"data_type".into())
    );
    assert_eq!(
        report.pointer("/enums/0/options/0/name"),
        Some(&"Huge".into())
    );
    Ok(())
}

#[test]
fn equal_schemas_exit_with_code_0() -> TestResult {
    let dir = pair(old_database(), old_database())?;

    let text = stdout(&mut schema(dir.path(), &[])?, 0)?;

    assert!(!text.contains("STRUCT"), "no struct table in\n{text}");
    Ok(())
}

#[test]
fn schema_cannot_be_combined_with_the_record_level() -> TestResult {
    let dir = pair(old_database(), new_database())?;

    let output = schema(dir.path(), &["--records"])?.output()?;

    assert_eq!(output.status.code(), Some(2), "usage error: {output:?}");
    Ok(())
}
