// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx dcb refs` over a synthetic DataForge: incoming and outgoing References with their
//! fields, unresolved References, the file graph, both output formats and invalid input, and,
//! with `--ignored`, over the real Archive `P4KX_DATA_P4K` names.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
    real_archive_path,
};

use serde_json::{Map, Value};

#[path = "dcb_refs_origins.rs"]
mod origins;

type TestResult = Result<(), Box<dyn Error>>;

const TERRA: u128 = 0x10;
const AEGIS: u128 = 0x20;
const ALPHA: u128 = 0x31;
const BETA: u128 = 0x32;
const GAMMA: u128 = 0x33;
const MISSING: u128 = 0x99;

fn reference_to(name: &str) -> TestType {
    TestType::ReferenceTo(name.to_owned())
}

fn ship(guid: u128, name: &str, path: &str) -> TestRecord {
    TestRecord::new(guid, "Ship", format!("Ship.{name}"), path)
}

fn database() -> Vec<u8> {
    original_database().build()
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Planet").field("name", TestType::String))
        .structure(TestStruct::new("Manufacturer").field("home", reference_to("Planet")))
        .structure(
            TestStruct::new("Ship")
                .field("maker", reference_to("Manufacturer"))
                .array("escorts", reference_to("Ship")),
        )
}

fn original_database() -> DataForgeBuilder {
    schema()
        .record(TestRecord::new(
            TERRA,
            "Planet",
            "Planet.Terra",
            "libs/planets/terra.xml",
        ))
        .record(
            TestRecord::new(
                AEGIS,
                "Manufacturer",
                "Manufacturer.Aegis",
                "libs/makers/aegis.xml",
            )
            .value("home", TestValue::Reference(TERRA)),
        )
        .record(
            ship(ALPHA, "Alpha", "libs/ships/alpha.xml")
                .value("maker", TestValue::Reference(AEGIS))
                .value(
                    "escorts",
                    TestValue::Array(vec![
                        TestValue::Null,
                        TestValue::Reference(BETA),
                        TestValue::Reference(MISSING),
                    ]),
                ),
        )
        .record(
            ship(BETA, "Beta", "libs/ships/beta.xml").value("maker", TestValue::Reference(AEGIS)),
        )
        .record(
            ship(GAMMA, "Gamma", "libs/ships/beta.xml")
                .value("maker", TestValue::Reference(MISSING)),
        )
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let archive = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database()).encrypted())
        .write_to(&archive)?;
    Ok(archive)
}

fn refs(args: &[&str], code: i32) -> Result<(String, String), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "refs"])
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
    let (stdout, stderr) = refs(args, 0)?;
    assert!(stderr.is_empty(), "no warnings: {stderr}");
    Ok(stdout)
}

fn assert_json_payload(args: &[&str], snapshot: &str) -> TestResult {
    let output = stdout(args)?;
    assert!(
        output.ends_with('\n') && output.lines().count() == 1,
        "the structured command emits one JSON line"
    );
    let mut actual: Value = serde_json::from_str(&output)?;
    strip_reference_origins(&mut actual)?;
    let payload = snapshot
        .lines()
        .skip(1)
        .skip_while(|line| *line != "---")
        .nth(1)
        .ok_or("original JSON snapshot payload")?;
    assert_eq!(
        actual,
        serde_json::from_str::<Value>(payload)?,
        "the original JSON payload is preserved"
    );
    Ok(())
}

fn strip_reference_origins(document: &mut Value) -> TestResult {
    let envelope = document.as_object_mut().ok_or("actual refs JSON object")?;
    envelope.remove("fieldOrigins");
    envelope.remove("provenance");
    for field in ["edges", "unresolved"] {
        strip_row_origins(envelope, field)?;
    }
    if let Some(records) = envelope.get_mut("records") {
        for row in records.as_array_mut().ok_or("actual records array")? {
            let listing = row.as_object_mut().ok_or("actual Record listing")?;
            listing.remove("fieldOrigins");
            for field in ["references", "unresolved"] {
                strip_row_origins(listing, field)?;
            }
        }
    }
    Ok(())
}

fn strip_row_origins(parent: &mut Map<String, Value>, field: &str) -> TestResult {
    if let Some(rows) = parent.get_mut(field) {
        for row in rows.as_array_mut().ok_or("actual refs rows")? {
            row.as_object_mut()
                .ok_or("actual refs row")?
                .remove("fieldOrigins");
        }
    }
    Ok(())
}

#[test]
fn incoming_lists_the_referring_records_with_their_fields() -> TestResult {
    insta::assert_snapshot!(stdout(&["libs/makers/aegis", "--incoming"])?);
    Ok(())
}

#[test]
fn a_guid_names_the_record_like_its_record_path() -> TestResult {
    let by_guid = stdout(&["00000000-0000-0000-0000-000000000020", "--incoming"])?;
    let by_path = stdout(&["LIBS\\MAKERS\\Aegis", "--incoming"])?;
    assert_eq!(by_guid, by_path);
    Ok(())
}

#[test]
fn outgoing_lists_the_referred_records_and_the_unresolved_ones_apart() -> TestResult {
    insta::assert_snapshot!(stdout(&["libs/ships/alpha", "--outgoing"])?);
    Ok(())
}

#[test]
fn a_record_path_that_several_records_share_lists_each_of_them() -> TestResult {
    insta::assert_snapshot!(stdout(&["libs/ships/beta", "--outgoing"])?);
    Ok(())
}

#[test]
fn incoming_as_json_keeps_field_path_and_indices() -> TestResult {
    assert_json_payload(
        &["libs/ships/beta", "--incoming", "--json"],
        include_str!(concat!(
            "snapshots/it__dcb_refs__",
            "incoming_as_json_keeps_field_path_and_indices.snap"
        )),
    )
}

#[test]
fn outgoing_as_json_lists_unresolved_references_separately() -> TestResult {
    assert_json_payload(
        &[
            "00000000-0000-0000-0000-000000000031",
            "--outgoing",
            "--format",
            "json",
        ],
        include_str!(concat!(
            "snapshots/it__dcb_refs__",
            "outgoing_as_json_lists_unresolved_references_separately.snap"
        )),
    )
}

#[test]
fn files_lists_the_edges_between_record_files() -> TestResult {
    insta::assert_snapshot!(stdout(&["--files"])?);
    Ok(())
}

#[test]
fn files_with_a_record_keeps_the_edges_of_its_file() -> TestResult {
    insta::assert_snapshot!(stdout(&["libs/planets/terra", "--files"])?);
    Ok(())
}

#[test]
fn files_as_json_counts_files_and_edges() -> TestResult {
    assert_json_payload(
        &["--files", "--json"],
        include_str!("snapshots/it__dcb_refs__files_as_json_counts_files_and_edges.snap"),
    )
}

#[test]
fn unresolved_lists_every_reference_to_a_missing_guid() -> TestResult {
    insta::assert_snapshot!(stdout(&["--unresolved"])?);
    assert_json_payload(
        &["--unresolved", "--json"],
        include_str!("snapshots/it__dcb_refs__unresolved_json.snap"),
    )
}

#[test]
fn a_record_nobody_refers_to_has_an_empty_listing() -> TestResult {
    insta::assert_snapshot!(stdout(&["libs/ships/alpha", "--incoming"])?);
    Ok(())
}

#[test]
fn an_unknown_record_is_invalid_input() -> TestResult {
    let (stdout, stderr) = refs(&["libs/nowhere", "--incoming"], 2)?;
    assert!(stdout.is_empty(), "nothing is printed: {stdout}");
    assert!(stderr.contains("libs/nowhere"), "{stderr}");
    Ok(())
}

#[test]
fn json_with_a_table_is_invalid_input() -> TestResult {
    let (_, stderr) = refs(&["--files", "--json", "--format", "table"], 2)?;
    assert!(stderr.contains("--json"), "{stderr}");
    Ok(())
}

#[test]
fn exactly_one_mode_and_a_record_for_incoming_are_required() -> TestResult {
    refs(&["libs/makers/aegis"], 2)?;
    refs(&["--incoming"], 2)?;
    refs(&["libs/makers/aegis", "--incoming", "--outgoing"], 2)?;
    refs(&["libs/makers/aegis", "--unresolved"], 2)?;
    Ok(())
}

#[test]
fn a_query_selects_the_records_that_refer_to_a_record() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let run = |query: &str, code: i32| -> Result<String, Box<dyn Error>> {
        let output = p4kx_testkit::p4kx()?
            .args(["dcb", "query"])
            .arg(&archive)
            .arg(query)
            .output()?;
        assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
        Ok(String::from_utf8(if code == 0 {
            output.stdout
        } else {
            output.stderr
        })?)
    };
    insta::assert_snapshot!(run(r#"from Ship referencing "libs/makers/aegis""#, 0)?);
    insta::assert_snapshot!(
        "unknown_referencing",
        run(r#"referencing "libs/nowhere""#, 2)?
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_real_archive_has_plausible_counts_and_incoming_references_of_a_ship() -> TestResult {
    let archive = real_archive_path()?;
    let run = |args: &[&str]| -> Result<Value, Box<dyn Error>> {
        let output = p4kx_testkit::p4kx()?
            .args(["dcb", "refs"])
            .arg(&archive)
            .args(args)
            .arg("--json")
            .output()?;
        assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
        Ok(serde_json::from_slice(&output.stdout)?)
    };
    let files = run(&["--files"])?;
    let edges = files["edge_count"].as_u64().ok_or("no edge_count")?;
    assert!(
        (100_000..2_000_000).contains(&edges),
        "a plausible number of file edges: {edges}"
    );
    let unresolved = run(&["--unresolved"])?["count"]
        .as_u64()
        .ok_or("no count")?;
    assert!(
        unresolved > 0 && unresolved < 10_000,
        "few unresolved References: {unresolved}"
    );
    let ship = run(&[
        "libs/foundry/records/entities/spaceships/aegs_avenger_stalker",
        "--incoming",
    ])?;
    let count = ship["records"][0]["reference_count"]
        .as_u64()
        .ok_or("no reference_count")?;
    assert!(
        count > 0,
        "the Avenger Stalker is referred to by other Records"
    );
    Ok(())
}
