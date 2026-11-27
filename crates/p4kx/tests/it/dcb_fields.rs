// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx dcb fields` over a synthetic DataForge: all Records, a type, a Query, a path filter, the
//! three output formats and invalid input, and, with `--ignored`, over the real Archive
//! `P4KX_DATA_P4K` names.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, real_archive_path,
};

#[path = "dcb_fields_provenance.rs"]
mod provenance;

type TestResult = Result<(), Box<dyn Error>>;

fn weapon(rate: f32) -> TestValue {
    TestValue::Struct(TestInstance::new("WeaponParams").value("fireRate", TestValue::Single(rate)))
}

fn database() -> Vec<u8> {
    let ship =
        |guid: u128, path: &str| TestRecord::new(guid, "Ship", format!("Ship.S{guid}"), path);
    DataForgeBuilder::new()
        .structure(TestStruct::new("Params"))
        .structure(
            TestStruct::new("WeaponParams")
                .parent("Params")
                .field("fireRate", TestType::Single),
        )
        .structure(
            TestStruct::new("Ship")
                .field("name", TestType::String)
                .field("crew", TestType::Int32)
                .field("mass", TestType::Double)
                .array("Components", TestType::StrongPointer("Params".to_owned())),
        )
        .structure(TestStruct::new("Crate").field("name", TestType::String))
        .record(
            ship(1, "libs/ships/one.xml")
                .value("name", TestValue::text("a, \"b\""))
                .value("crew", TestValue::Int32(0))
                .value("mass", TestValue::Double(-0.0))
                .value(
                    "Components",
                    TestValue::Array(vec![weapon(2.0), weapon(6.5)]),
                ),
        )
        .record(
            ship(2, "libs/ships/two.xml")
                .value("name", TestValue::text(""))
                .value("crew", TestValue::Int32(4))
                .value("mass", TestValue::Double(0.0))
                .value("Components", TestValue::Array(vec![weapon(-1.5)])),
        )
        .record(
            TestRecord::new(3, "Crate", "Crate.One", "libs/crates/one.xml")
                .value("name", TestValue::text("box")),
        )
        .build()
}

fn write_archive(dir: &Path, forge: Vec<u8>) -> Result<PathBuf, Box<dyn Error>> {
    let archive = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge).encrypted())
        .write_to(&archive)?;
    Ok(archive)
}

fn fields(args: &[&str], code: i32) -> Result<(String, String), Box<dyn Error>> {
    fields_of(database(), args, code)
}

fn fields_of(forge: Vec<u8>, args: &[&str], code: i32) -> Result<(String, String), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), forge)?;
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "fields"])
        .arg(&archive)
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    let archive_text = archive.display().to_string();
    Ok((
        String::from_utf8(output.stdout)?
            .replace(&archive_text.replace('\\', "\\\\"), "<archive>")
            .replace(&archive_text, "<archive>"),
        String::from_utf8(output.stderr)?,
    ))
}

fn stdout(args: &[&str]) -> Result<String, Box<dyn Error>> {
    let (stdout, stderr) = fields(args, 0)?;
    assert!(stderr.is_empty(), "no warnings: {stderr}");
    Ok(stdout)
}

#[test]
fn every_record_is_counted_by_default() -> TestResult {
    insta::assert_snapshot!(stdout(&[])?);
    Ok(())
}

#[test]
fn a_type_name_selects_the_records_of_that_type() -> TestResult {
    insta::assert_snapshot!(stdout(&["Crate"])?);
    let ships = stdout(&["Ship"])?;
    assert!(
        ships.contains("Components/WeaponParams/fireRate"),
        "{ships}"
    );
    assert!(ships.ends_with("in 2 Records\n"), "{ships}");
    Ok(())
}

#[test]
fn a_query_selects_by_type_record_path_and_condition() -> TestResult {
    let by_path = stdout(&["from Ship at \"libs/ships/t*\""])?;
    assert!(by_path.ends_with("in 1 Record\n"), "{by_path}");
    let by_condition = stdout(&["from Ship select name where crew > 0"])?;
    assert!(by_condition.ends_with("in 1 Record\n"), "{by_condition}");
    assert!(
        by_condition.contains("Components/WeaponParams"),
        "{by_condition}"
    );
    Ok(())
}

#[test]
fn a_path_glob_keeps_the_matching_paths() -> TestResult {
    insta::assert_snapshot!(stdout(&["--path", "Components/WeaponParams/*"])?);
    Ok(())
}

#[test]
fn csv_quotes_like_rfc_4180() -> TestResult {
    let csv = stdout(&["Ship", "--format", "csv"])?;
    assert!(
        csv.starts_with("\u{feff}path,kinds,values,records,empty,distinct,min,max\r\n"),
        "{csv}"
    );
    insta::assert_snapshot!(csv);
    Ok(())
}

fn formula_database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Sheet").field("=SUM(A1)", TestType::Int32))
        .record(
            TestRecord::new(1, "Sheet", "Sheet.One", "libs/sheet/one.xml")
                .value("=SUM(A1)", TestValue::Int32(-5)),
        )
        .build()
}

#[test]
fn csv_starts_with_a_byte_order_mark_and_neutralises_a_formula_path() -> TestResult {
    let (csv, _) = fields_of(formula_database(), &["--format", "csv"], 0)?;
    assert!(csv.starts_with('\u{feff}'), "{csv:?}");
    assert!(
        csv.contains("\r\n'=SUM(A1),Int32,1,1,0,1,-5,-5\r\n"),
        "{csv:?}"
    );
    Ok(())
}

#[test]
fn csv_raw_writes_the_exact_paths_without_a_byte_order_mark() -> TestResult {
    let (csv, _) = fields_of(formula_database(), &["--format", "csv", "--csv-raw"], 0)?;
    assert!(csv.starts_with("path,"), "{csv:?}");
    assert!(
        csv.contains("\r\n=SUM(A1),Int32,1,1,0,1,-5,-5\r\n"),
        "{csv:?}"
    );
    Ok(())
}

#[test]
fn csv_raw_needs_the_csv_format() -> TestResult {
    let (_, stderr) = fields(&["--csv-raw"], 2)?;
    assert!(stderr.contains("--csv-raw"), "{stderr}");
    Ok(())
}

#[test]
fn a_type_name_with_a_backtick_is_invalid_input() -> TestResult {
    let (_, stderr) = fields(&["Sh`ip"], 2)?;
    assert!(stderr.contains("backtick"), "{stderr}");
    Ok(())
}

#[test]
fn more_paths_than_the_limit_exit_with_code_5() -> TestResult {
    let (stdout, stderr) = fields_of(p4kx_testkit::many_paths_database(33, 1_000), &[], 5)?;
    assert!(stdout.is_empty(), "nothing is printed: {stdout}");
    assert!(stderr.contains("paths"), "{stderr}");
    Ok(())
}

#[test]
fn json_has_typed_numbers_and_kinds_with_counts() -> TestResult {
    let text = stdout(&["Ship", "--json"])?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(json["archive"], "<archive>", "archive");
    assert_eq!(json["records"], 2, "records");
    assert_eq!(json["unreadable_records"], 0, "unreadable");
    let list = json["fields"].as_array().ok_or("no fields")?;
    assert_eq!(json["field_count"], list.len(), "field_count");
    let field = |path: &str| -> Result<&serde_json::Value, String> {
        list.iter()
            .find(|field| field["path"] == path)
            .ok_or_else(|| format!("no {path}: {text}"))
    };
    let crew = field("crew")?;
    assert_eq!(crew["min"], 0, "min crew");
    assert_eq!(crew["max"], 4, "max crew");
    assert_eq!(crew["kinds"][0]["kind"], "Int32", "kind");
    assert_eq!(crew["kinds"][0]["count"], 2, "count");
    assert_eq!(field("mass")?["distinct"], 1, "-0.0 and 0.0 are one value");
    assert_eq!(field("name")?["empty"], 1, "the empty name");
    assert_eq!(
        field("name")?["min"],
        serde_json::Value::Null,
        "text has no number"
    );
    assert_eq!(
        field("Components/WeaponParams/fireRate")?["min"],
        -1.5,
        "min rate"
    );
    assert_eq!(
        field("Components")?["kinds"][0]["kind"],
        "StrongPointer[]",
        "array kind"
    );
    let same = stdout(&["Ship", "--format", "json"])?;
    assert_eq!(same, text, "--json is --format json");
    Ok(())
}

#[test]
fn an_unknown_type_is_invalid_input_with_the_position() -> TestResult {
    let (stdout, stderr) = fields(&["Shpi"], 2)?;
    assert!(stdout.is_empty(), "nothing is printed: {stdout}");
    assert!(stderr.contains("unknown type"), "{stderr}");
    assert!(stderr.contains('^'), "the position is marked: {stderr}");
    Ok(())
}

#[test]
fn a_query_with_a_syntax_error_is_invalid_input() -> TestResult {
    let (_, stderr) = fields(&["from Ship where"], 2)?;
    assert!(stderr.contains('^'), "the position is marked: {stderr}");
    Ok(())
}

#[test]
fn json_with_another_format_is_invalid_input() -> TestResult {
    let (_, stderr) = fields(&["--json", "--format", "csv"], 2)?;
    assert!(stderr.contains("--json"), "{stderr}");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_real_archive_counts_a_known_field() -> TestResult {
    let archive = real_archive_path()?;
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "fields"])
        .arg(&archive)
        .args(["EntityClassDefinition", "--path", "Components*", "--json"])
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let records = json["records"].as_u64().ok_or("no records")?;
    assert!(records > 1_000, "plenty of entities: {records}");
    let list = json["fields"].as_array().ok_or("no fields")?;
    let components = list
        .iter()
        .find(|field| field["path"] == "Components")
        .ok_or("no Components")?;
    assert_eq!(
        components["kinds"][0]["kind"], "StrongPointer[]",
        "component pointers"
    );
    let rate = list
        .iter()
        .find(|field| {
            field["path"]
                == "Components/SCItemWeaponComponentParams/fireActions/SWeaponActionFireRapidParams/fireRate"
        })
        .ok_or("no fire rate of rapid-fire weapons")?;
    assert!(
        rate["records"].as_u64().unwrap_or(0) > 50,
        "many rapid-fire weapons: {rate}"
    );
    assert_eq!(rate["kinds"][0]["kind"], "Int32", "fire rates are integers");
    let max = rate["max"].as_i64().ok_or("no largest fire rate")?;
    let min = rate["min"].as_i64().ok_or("no smallest fire rate")?;
    assert!(
        0 < min && min < max && max < 100_000,
        "plausible fire rates: {min} to {max}"
    );
    Ok(())
}
