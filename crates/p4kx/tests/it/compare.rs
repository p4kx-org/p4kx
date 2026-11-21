// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx compare` end to end over pairs of synthetic Archives.

use std::error::Error;
use std::path::Path;

use assert_cmd::Command;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntry, TestRecord, TestStruct, TestType,
    write_build_manifest,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const OLD: &str = "LIVE/Data.p4k";
const NEW: &str = "PTU/Data.p4k";

fn write(dir: &Path, archive: &str, builder: &ArchiveBuilder) -> TestResult {
    let path = dir.join(archive);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    builder.write_to(path)?;
    Ok(())
}

fn dataforge() -> TestEntry {
    TestEntry::stored("Data\\Game2.dcb", DataForgeBuilder::new().build())
}

fn old_archive() -> ArchiveBuilder {
    ArchiveBuilder::new()
        .entry(dataforge())
        .entry(TestEntry::stored("Data\\same.txt", b"same".to_vec()))
        .entry(TestEntry::stored("Data\\removed.txt", b"gone".to_vec()))
        .entry(TestEntry::stored("Data\\Libs\\size.xml", b"<a/>".to_vec()))
        .entry(TestEntry::stored("Data\\Libs\\crc.xml", b"<a/>".to_vec()))
        .entry(TestEntry::deflated(
            "Engine\\recompressed.cfg",
            b"x=1".to_vec(),
        ))
}

fn pair() -> Result<tempfile::TempDir, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    write(dir.path(), OLD, &old_archive())?;
    write(
        dir.path(),
        NEW,
        &ArchiveBuilder::v2()
            .entry(dataforge())
            .entry(TestEntry::zstd("Engine\\recompressed.cfg", b"x=1".to_vec()))
            .entry(TestEntry::stored("DATA\\LIBS\\CRC.xml", b"<b/>".to_vec()))
            .entry(TestEntry::stored("Data\\Libs\\size.xml", b"<ab/>".to_vec()))
            .entry(TestEntry::stored("Data\\added.txt", b"new".to_vec()))
            .entry(TestEntry::stored("data\\same.txt", b"same".to_vec()))
            .entry(TestEntry::stored("root.txt", b"r".to_vec())),
    )?;
    write_build_manifest(
        &dir.path().join("PTU"),
        "sc-alpha-4.11.0",
        "4.11.42.1234",
        "12999999",
        "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0",
    )?;
    Ok(dir)
}

fn p4kx(dir: &Path) -> Result<Command, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command.current_dir(dir).arg("compare");
    Ok(command)
}

fn stdout(command: &mut Command, code: i32) -> Result<String, Box<dyn Error>> {
    let output = command.output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn differing_archives_print_a_summary_and_exit_with_code_1() -> TestResult {
    let dir = pair()?;

    let summary = stdout(p4kx(dir.path())?.args([OLD, NEW, "--entries"]), 1)?;

    insta::assert_snapshot!(summary);
    Ok(())
}

#[test]
fn differing_checksum_algorithms_report_an_incomparable_entry_and_exit_with_code_1() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = "Data/same.txt";
    let body = b"123456789";
    std::fs::write(
        dir.path().join("before.zip"),
        TestContainer::new()
            .entry(TestEntry::stored(path, body))
            .build(),
    )?;
    write(
        dir.path(),
        NEW,
        &ArchiveBuilder::new().entry(TestEntry::stored(path, body)),
    )?;
    let stdout = stdout(
        p4kx(dir.path())?.args(["before.zip", NEW, "--entries", "--json"]),
        1,
    )?;
    let report: Value = serde_json::from_str(&stdout)?;
    assert_eq!(
        report.pointer("/entries/counts/changed"),
        Some(&json!(0)),
        "confirmed changes"
    );
    assert_eq!(
        report.pointer("/entries/incomparable/0/reason"),
        Some(&json!("checksum_algorithms_differ")),
        "incomplete comparison result"
    );
    Ok(())
}

#[test]
fn identical_archives_exit_with_code_0() -> TestResult {
    let dir = pair()?;

    let summary = stdout(p4kx(dir.path())?.args([OLD, OLD]), 0)?;

    assert!(
        summary.contains("Added:           0\n"),
        "nothing added: {summary}"
    );
    Ok(())
}

#[test]
fn the_report_holds_both_sides_the_levels_the_filter_and_the_entry_changes() -> TestResult {
    let dir = pair()?;

    stdout(
        p4kx(dir.path())?.args([
            OLD,
            NEW,
            "--entries",
            "--filter",
            "data/*",
            "--report",
            "report.json",
        ]),
        1,
    )?;

    let report = std::fs::read_to_string(dir.path().join("report.json"))?;
    insta::assert_snapshot!(report);
    Ok(())
}

#[test]
fn the_record_report_keeps_bound_input_proof_in_json_and_saved_report() -> TestResult {
    let directory = tempfile::tempdir()?;
    for (path, name) in [(OLD, "Original"), (NEW, "Updated")] {
        let forge = DataForgeBuilder::new()
            .structure(TestStruct::new("Ship").field("speed", TestType::Int32))
            .record(TestRecord::new(1, "Ship", name, "ship.xml"))
            .build();
        let archive = ArchiveBuilder::new().entry(TestEntry::stored("Data\\Game2.dcb", forge));
        write(directory.path(), path, &archive)?;
    }

    let output = stdout(
        p4kx(directory.path())?.args([OLD, NEW, "--records", "--report", "report.json", "--json"]),
        1,
    )?;
    let printed: Value = serde_json::from_str(&output)?;
    let saved: Value =
        serde_json::from_reader(std::fs::File::open(directory.path().join("report.json"))?)?;
    assert_eq!(
        printed, saved,
        "record reports share the same physical-sources carrier"
    );
    let origins = printed
        .pointer("/records/fieldOrigins")
        .ok_or("record header origins")?;
    for side in ["old", "new"] {
        let header = origins
            .get(format!("/records/changes/0/{side}/name"))
            .ok_or("present name field source")?;
        assert_eq!(
            header.get("containsEncryptedSource"),
            Some(&json!(false)),
            "the {side} header carries clear DataForge input proof",
        );
    }
    Ok(())
}

#[test]
fn the_report_is_deterministic() -> TestResult {
    let dir = pair()?;
    let mut reports = Vec::new();
    for _ in 0..3 {
        stdout(
            p4kx(dir.path())?.args([OLD, NEW, "--report", "report.json"]),
            1,
        )?;
        reports.push(std::fs::read(dir.path().join("report.json"))?);
    }

    assert!(
        reports.windows(2).all(|pair| pair.first() == pair.get(1)),
        "every run writes the same bytes"
    );
    Ok(())
}

#[test]
fn json_prints_the_report_as_one_line() -> TestResult {
    let dir = pair()?;

    let line = stdout(p4kx(dir.path())?.args(["--json", OLD, NEW]), 1)?;

    assert_eq!(line.lines().count(), 1, "one line: {line}");
    let report: Value = serde_json::from_str(&line)?;
    assert_eq!(
        report.pointer("/entries/counts"),
        Some(&serde_json::json!({"added": 2, "removed": 1, "changed": 2})),
        "counts: {report}"
    );
    Ok(())
}

#[test]
fn a_closed_stdout_keeps_the_exit_code_of_the_result() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(dir.path(), OLD, &old_archive())?;
    let new = (0..5000).fold(old_archive(), |builder, index| {
        builder.entry(TestEntry::stored(
            format!("Data\\Added\\entry-{index:05}.xml"),
            b"<a/>".to_vec(),
        ))
    });
    write(dir.path(), NEW, &new)?;

    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("p4kx"))
        .current_dir(dir.path())
        .args(["--json", "compare", OLD, NEW])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    drop(child.stdout.take());
    let output = child.wait_with_output()?;

    assert_eq!(
        output.status.code(),
        Some(1),
        "differences, not an error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn a_file_that_is_not_an_archive_exits_with_code_3() -> TestResult {
    let dir = pair()?;
    std::fs::write(dir.path().join("notes.txt"), "plain text")?;

    stdout(p4kx(dir.path())?.args([OLD, "notes.txt"]), 3)?;
    Ok(())
}

#[test]
fn a_missing_argument_exits_with_code_2() -> TestResult {
    let dir = pair()?;

    stdout(p4kx(dir.path())?.arg(OLD), 2)?;
    Ok(())
}

#[test]
fn a_report_that_cannot_be_written_exits_with_code_6() -> TestResult {
    let dir = pair()?;

    let output = p4kx(dir.path())?
        .args([OLD, NEW, "--report", "missing/report.json"])
        .output()?;

    assert_eq!(output.status.code(), Some(6), "exit code: {output:?}");
    assert!(
        String::from_utf8(output.stderr)?.contains("cannot write the report"),
        "the error names the report"
    );
    Ok(())
}
