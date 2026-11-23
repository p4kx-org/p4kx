// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx dcb query --format table|csv|json [--out <file>]` end to end over a synthetic
//! DataForge and, with `--ignored`, over the real Archive `P4KX_DATA_P4K` names.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
    real_archive_path,
};

type TestResult = Result<(), Box<dyn Error>>;

const QUERY: &str = "at \"*\" select name, crew, mass, size, docked";

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Large"])
        .structure(
            TestStruct::new("Item")
                .field("name", TestType::String)
                .field("mass", TestType::Single)
                .field("size", TestType::Enum("Size".to_owned())),
        )
        .structure(
            TestStruct::new("Ship")
                .parent("Item")
                .field("crew", TestType::Int32)
                .field("docked", TestType::Bool),
        )
        .record(
            TestRecord::new(1, "Ship", "Ship.Avenger", "libs/ships/avenger.xml")
                .value("name", TestValue::text("Avenger, \"Titan\""))
                .value("mass", TestValue::Single(25_000.5))
                .value("size", TestValue::text("Large"))
                .value("crew", TestValue::Int32(2))
                .value("docked", TestValue::Bool(false)),
        )
        .record(
            TestRecord::new(2, "Item", "Item.Note", "libs/items/note.xml")
                .value("name", TestValue::text("two\nlines"))
                .value("mass", TestValue::Single(f32::NAN))
                .value("size", TestValue::Raw(vec![0xff; 4])),
        )
        .record(
            TestRecord::new(3, "Item", "Item.Box", "libs/items/box.xml")
                .value("name", TestValue::text("Box"))
                .value("mass", TestValue::Single(-0.25))
                .value("size", TestValue::text("Small")),
        )
}

fn write_archive(dir: &Path, forge: &DataForgeBuilder) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge.build()))
        .write_to(&path)?;
    Ok(path)
}

fn run(archive: &Path, flags: &[&str], query: &str) -> Result<Output, Box<dyn Error>> {
    let (global, local): (Vec<&str>, Vec<&str>) = flags.iter().partition(|flag| **flag == "--json");
    Ok(p4kx_testkit::p4kx()?
        .args(global)
        .args(["dcb", "query"])
        .arg(archive)
        .arg(query)
        .args(local)
        .output()?)
}

fn stdout(flags: &[&str]) -> Result<String, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &database())?;
    let output = run(&archive, flags, QUERY)?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    assert!(output.stderr.is_empty(), "no warnings: {output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn csv_is_rfc_4180_with_a_header_row() -> TestResult {
    let csv = stdout(&["--format", "csv"])?;

    assert_eq!(
        csv.matches("\r\n").count(),
        4,
        "CRLF after the header and each row: {csv:?}"
    );
    assert!(csv.ends_with("\r\n"), "CRLF at the end: {csv:?}");
    insta::assert_snapshot!(csv.replace("\r\n", "<CRLF>\n"));
    Ok(())
}

#[test]
fn json_is_an_array_of_objects_with_typed_values() -> TestResult {
    let json = stdout(&["--format", "json"])?;

    let rows: serde_json::Value = serde_json::from_str(&json)?;
    assert_eq!(json.lines().count(), 1, "one line: {json}");
    assert_eq!(
        rows.as_array().map(Vec::len),
        Some(3),
        "one object per row: {json}"
    );
    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn table_is_the_default_format() -> TestResult {
    let table = stdout(&["--format", "table"])?;

    assert_eq!(table, stdout(&[])?, "--format table is the default");
    insta::assert_snapshot!(table);
    Ok(())
}

#[test]
fn the_json_flag_writes_the_result_as_json() -> TestResult {
    assert_eq!(
        stdout(&["--json"])?,
        stdout(&["--format", "json"])?,
        "--json without --format means json"
    );
    Ok(())
}

#[test]
fn columns_follow_the_select_clause_and_rows_the_record_path() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &database())?;
    let output = run(
        &archive,
        &["--format", "csv"],
        "from Item select size, name",
    )?;

    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "\u{feff}record path,size,name\r\nlibs/items/box,Small,Box\r\nlibs/items/note,error: offset 4294967295 in the text table is not a valid string,\"two\nlines\"\r\nlibs/ships/avenger,Large,\"Avenger, \"\"Titan\"\"\"\r\n",
        "select order, record path order"
    );
    Ok(())
}

#[test]
fn a_where_condition_filters_the_csv_and_json_rows() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &database())?;
    let text = "from Item select name, mass where mass > 0";

    let csv = run(&archive, &["--format", "csv"], text)?;
    let json = run(&archive, &["--format", "json"], text)?;

    assert_eq!(csv.status.code(), Some(0), "csv: {csv:?}");
    assert_eq!(json.status.code(), Some(0), "json: {json:?}");
    assert_eq!(
        String::from_utf8(csv.stdout)?,
        "\u{feff}record path,name,mass\r\nlibs/ships/avenger,\"Avenger, \"\"Titan\"\"\",25000.5\r\n",
        "only the Record that meets the condition"
    );
    assert_eq!(
        String::from_utf8(json.stdout)?,
        "[{\"record path\":\"libs/ships/avenger\",\"name\":\"Avenger, \\\"Titan\\\"\",\"mass\":25000.5}]\n",
        "only the Record that meets the condition"
    );
    Ok(())
}

#[test]
fn out_writes_the_result_to_a_file_and_a_summary_to_stdout() -> TestResult {
    for (format, expected) in [
        ("csv", stdout(&["--format", "csv"])?),
        ("csv-strict", stdout(&["--format", "csv-strict"])?),
        ("json", stdout(&["--format", "json"])?),
        (
            "table",
            stdout(&["--format", "table"])?.replace("3 Records\n", ""),
        ),
    ] {
        let dir = tempfile::tempdir()?;
        let archive = write_archive(dir.path(), &database())?;
        let file = dir.path().join(format!("result.{format}"));
        let file_text = file.display().to_string();
        let output = run(&archive, &["--format", format, "--out", &file_text], QUERY)?;

        assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
        assert_eq!(
            String::from_utf8(output.stdout)?,
            format!("3 Records written to {file_text}\n"),
            "{format} summary"
        );
        assert_eq!(fs::read_to_string(&file)?, expected, "{format} file");
        let leftovers = fs::read_dir(dir.path())?.count();
        assert_eq!(leftovers, 2, "only the Archive and the result remain");
    }
    Ok(())
}

#[test]
fn out_replaces_an_existing_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &database())?;
    let file = dir.path().join("result.csv");
    fs::write(
        &file,
        "old content that is longer than the new one ".repeat(20),
    )?;
    let file_text = file.display().to_string();

    let output = run(&archive, &["--format", "csv", "--out", &file_text], QUERY)?;

    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    assert_eq!(
        fs::read_to_string(&file)?,
        stdout(&["--format", "csv"])?,
        "replaced"
    );
    Ok(())
}

#[test]
fn csv_raw_writes_no_byte_order_mark_and_needs_the_csv_format() -> TestResult {
    let raw = stdout(&["--format", "csv", "--csv-raw"])?;
    let default = stdout(&["--format", "csv"])?;
    assert_eq!(default.strip_prefix('\u{feff}'), Some(raw.as_str()), "mark");
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &database())?;
    let output = run(&archive, &["--csv-raw"], QUERY)?;
    assert_eq!(output.status.code(), Some(2), "exit code: {output:?}");
    Ok(())
}

#[test]
fn json_with_out_prints_a_json_summary() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &database())?;
    let file = dir.path().join("result.csv");
    let file_text = file.display().to_string();

    let output = run(
        &archive,
        &["--json", "--format", "csv", "--out", &file_text],
        QUERY,
    )?;

    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        summary,
        serde_json::json!({
            "archive": archive.display().to_string(),
            "format": "csv",
            "output": file_text,
            "records": 3,
            "failed": 0,
            "unresolved_references": 0,
        }),
        "summary"
    );
    assert!(
        fs::read_to_string(&file)?.starts_with("\u{feff}record path,"),
        "CSV in the file"
    );
    Ok(())
}

#[test]
fn json_with_another_format_on_stdout_is_rejected() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &database())?;
    for format in ["table", "csv"] {
        let output = run(&archive, &["--json", "--format", format], QUERY)?;

        assert_eq!(output.status.code(), Some(2), "exit code: {output:?}");
        assert!(output.stdout.is_empty(), "nothing on stdout: {output:?}");
        insta::assert_snapshot!(
            format!("json_with_{format}_on_stdout"),
            String::from_utf8(output.stderr)?
        );
    }
    Ok(())
}

#[test]
fn out_cannot_be_the_archive() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &database())?;
    let before = fs::read(&archive)?;
    let archive_text = archive.display().to_string();

    let output = run(
        &archive,
        &["--format", "csv", "--out", &archive_text],
        QUERY,
    )?;

    assert_eq!(output.status.code(), Some(2), "exit code: {output:?}");
    assert_eq!(fs::read(&archive)?, before, "the Archive is unchanged");
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("is the Archive"),
        "names the problem: {stderr}"
    );
    Ok(())
}

#[test]
fn table_columns_keep_their_width_across_pages() -> TestResult {
    let mut forge = DataForgeBuilder::new().structure(
        TestStruct::new("Item")
            .field("name", TestType::String)
            .field("n", TestType::Int32),
    );
    for index in 0..1100_i32 {
        let name = if index == 1099 {
            "w".repeat(40)
        } else {
            "x".to_owned()
        };
        forge = forge.record(
            TestRecord::new(
                u128::try_from(index)? + 1,
                "Item",
                format!("Item.R{index}"),
                format!("libs/items/r{index:04}.xml"),
            )
            .value("name", TestValue::text(name))
            .value("n", TestValue::Int32(index)),
        );
    }
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &forge)?;

    let output = run(&archive, &[], "from Item select name, n")?;

    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let table = String::from_utf8(output.stdout)?;
    let starts: Vec<usize> = table
        .lines()
        .filter(|line| line.starts_with("libs/") || line.starts_with("record path"))
        .filter_map(|line| line.rfind(' ').map(|space| space + 1))
        .collect();
    assert_eq!(starts.len(), 1101, "header and every row");
    assert!(
        starts.iter().all(|start| Some(start) == starts.first()),
        "the last column starts at one position: {:?}",
        starts.iter().collect::<std::collections::BTreeSet<_>>()
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_real_archive_gives_csv_and_json() -> TestResult {
    let archive = real_archive_path()?;
    let query = "from EntityClassDefinition at \"*/spaceships/*\" select Category, Invisible";

    let csv = run(&archive, &["--format", "csv"], query)?;
    let json = run(&archive, &["--format", "json"], query)?;

    assert_eq!(csv.status.code(), Some(0), "csv: {csv:?}");
    assert_eq!(json.status.code(), Some(0), "json: {json:?}");
    let csv = String::from_utf8(csv.stdout)?;
    let rows: serde_json::Value = serde_json::from_slice(&json.stdout)?;
    let rows = rows.as_array().ok_or("an array")?;
    assert!(
        csv.starts_with("\u{feff}record path,Category,Invisible\r\n"),
        "header: {csv}"
    );
    assert!(rows.len() > 1, "selects Records");
    assert_eq!(
        csv.matches("\r\n").count(),
        rows.len() + 1,
        "one CSV row per JSON object"
    );
    assert!(
        rows.iter().all(|row| row
            .get("Invisible")
            .is_some_and(serde_json::Value::is_boolean)),
        "booleans are typed"
    );
    Ok(())
}
