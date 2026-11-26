// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx compare --schema` end to end over pairs of synthetic DataForges.

use std::error::Error;
use std::path::Path;

use assert_cmd::Command;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

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
    write_archive(
        dir,
        archive,
        &ArchiveBuilder::new().entry(TestEntry::stored("Data\\Game2.dcb", dataforge)),
    )
}

fn write_archive(dir: &Path, archive: &str, builder: &ArchiveBuilder) -> TestResult {
    let path = dir.join(archive);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    builder.write_to(path)?;
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

    let report: Value = serde_json::from_str(&text)?;
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

fn origin_database(speed: TestType, traversal: Option<TestInstance>) -> Vec<u8> {
    let mut builder = DataForgeBuilder::new()
        .structure(TestStruct::new("TriggerBase").field("enabled", TestType::Bool))
        .structure(
            TestStruct::new("Ship")
                .field("speed", speed)
                .field("trigger", TestType::StrongPointer("TriggerBase".to_owned())),
        );
    let mut ship = TestRecord::new(SHIP, "Ship", "Ship.a", "libs/ships/a.xml");
    if let Some(traversal) = traversal {
        builder = builder.structure(
            TestStruct::new("TraversalTarget")
                .parent("TriggerBase")
                .field("nextTarget", TestType::StrongPointer("Ship".to_owned())),
        );
        ship = ship.value("trigger", TestValue::Struct(traversal));
    }
    builder.record(ship).build()
}

#[test]
fn schema_json_distinguishes_old_and_new_declarations_and_nested_mappings() -> TestResult {
    let dir = tempfile::tempdir()?;
    let old = origin_database(TestType::Int32, None);
    let new = origin_database(TestType::Single, Some(TestInstance::new("TraversalTarget")));
    let path = "Data\\Game2.dcb";
    write_archive(
        dir.path(),
        OLD,
        &ArchiveBuilder::v2().entry(TestEntry::zstd(path, old).encrypted()),
    )?;
    write_archive(
        dir.path(),
        NEW,
        &ArchiveBuilder::v2().entry(TestEntry::stored(path, new)),
    )?;
    let text = stdout(&mut schema(dir.path(), &["--json"])?, 1)?;
    let report: Value = serde_json::from_str(&text)?;
    check_schema_origins(&report)
}

fn check_schema_origins(report: &Value) -> TestResult {
    let old_type = "/structs/0/properties/0/old/data_type";
    let new_type = "/structs/0/properties/0/new/data_type";
    let target = "/structs/1/properties/0/new/target";
    let mappings = "/structs/1/mapped_instances/new";
    for (path, expected) in [
        ("/format_version", json!(1)),
        ("/structs/0/name", json!("Ship")),
        ("/structs/0/properties/0/name", json!("speed")),
        (old_type, json!("Int32")),
        (new_type, json!("Single")),
        ("/structs/1/name", json!("TraversalTarget")),
        ("/structs/1/change", json!("added")),
        (target, json!("Ship")),
        ("/structs/1/records/new", json!(0)),
        (mappings, json!(1)),
    ] {
        assert_eq!(
            report.pointer(path),
            Some(&expected),
            "schema value at {path}"
        );
    }

    let old_source = original_schema_source(report, old_type, true)?;
    let new_source = original_schema_source(report, new_type, false)?;
    assert_ne!(
        old_source, new_source,
        "same Entry path, two original inputs"
    );
    assert_eq!(
        original_schema_source(report, target, false)?,
        new_source,
        "the resolved target belongs to the new DataForge"
    );
    assert_eq!(
        original_schema_source(report, mappings, false)?,
        new_source,
        "the exact nested mapping belongs to the new DataForge"
    );
    Ok(())
}

fn original_schema_source<'a>(
    report: &'a Value,
    path: &str,
    encrypted: bool,
) -> Result<&'a str, Box<dyn Error>> {
    let origin = report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or_else(|| format!("origin for {path}"))?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "immediate encryption marker at {path}"
    );
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("original use inventory")?;
    let references = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("original use references")?;
    let mut sources = references.iter().map(|id| {
        uses.iter()
            .find(|used| used.get("id") == Some(id))
            .and_then(|used| used.get("source"))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("unresolved source use {id} at {path}"))
    });
    let source = sources
        .next()
        .ok_or_else(|| format!("no original use at {path}"))??;
    for contributor in sources {
        assert_eq!(
            contributor?, source,
            "every operand for {path} has the same side"
        );
    }
    let physical = report
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical source inventory")?;
    assert!(
        physical
            .iter()
            .any(|entry| entry.get("id").and_then(Value::as_str) == Some(source)),
        "original physical input {source} resolves for {path}"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(true)),
        "complete original operands for {path}"
    );
    Ok(source)
}
