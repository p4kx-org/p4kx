// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx grep` end to end over a synthetic Archive, and, with `--ignored`, over the real
//! Archive `P4KX_DATA_P4K` names.

use std::error::Error;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
    TestType, TestValue, real_archive_path,
};
use predicates::prelude::{PredicateBooleanExt, predicate};

#[path = "grep_origins_cases.rs"]
mod origins;

type TestResult = Result<(), Box<dyn Error>>;

fn forge() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Gear")
                .field("name", TestType::String)
                .array("tags", TestType::String),
        )
        .record(
            TestRecord::new(
                0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f,
                "Gear",
                "Gear.AEGS_Gladius",
                "libs/ships/aegs_gladius.xml",
            )
            .value("name", TestValue::text("AEGS_Gladius Fighter"))
            .value(
                "tags",
                TestValue::Array(vec![TestValue::text("alpha"), TestValue::text("a gladius")]),
            ),
        )
        .build()
}

fn builder() -> ArchiveBuilder {
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge()))
        .entry(TestEntry::stored(
            "Data\\Localization\\english\\global.ini",
            GlobalIniBuilder::new()
                .text("vehicle_NameAEGS_Gladius", "Aegis Gladius")
                .text("item_Other", "Nothing")
                .build(),
        ))
        .entry(TestEntry::deflated(
            "Data\\Scripts\\ship.xml",
            b"<Ship>\n  <Part name=\"AEGS_Gladius\"/>\n</Ship>\n".to_vec(),
        ))
        .entry(TestEntry::stored(
            "Data\\Scripts\\binary.xml",
            b"gladius\0\x01".to_vec(),
        ))
        .entry(TestEntry::stored("Data\\Scripts\\secret.xml", b"<gladius/>".to_vec()).encrypted())
}

fn write(dir: &Path, builder: &ArchiveBuilder) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    builder.write_to(&path)?;
    Ok(path)
}

fn keyless(dir: &Path) -> Result<Command, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .env_remove("P4KX_ARCHIVE_KEY")
        .env("P4KX_KEY_FILE", dir.join("profile").join("archive.key"));
    Ok(command)
}

#[test]
fn a_table_lists_the_hits_of_every_source() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write(dir.path(), &builder())?;
    let output = p4kx_testkit::p4kx()?
        .arg("grep")
        .arg(&archive)
        .arg("gladi")
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    let lines: Vec<&str> = stdout.lines().map(str::trim_end).collect();
    assert_eq!(
        lines,
        [
            "RECORD PATH              RECORD             FIELD    TEXT",
            "-----------------------  -----------------  -------  --------------------",
            "libs/ships/aegs_gladius  Gear.AEGS_Gladius  name     AEGS_Gladius Fighter",
            "libs/ships/aegs_gladius  Gear.AEGS_Gladius  tags[1]  a gladius",
            "",
            "LANGUAGE  KEY                       TEXT",
            "--------  ------------------------  -------------",
            "en        vehicle_NameAEGS_Gladius  Aegis Gladius",
            "",
            "ENTRY:LINE                 TEXT",
            "-------------------------  -----------------------------",
            "Data\\Scripts\\secret.xml:1  <gladius/>",
            "Data\\Scripts\\ship.xml:2      <Part name=\"AEGS_Gladius\"/>",
        ],
        "stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("1 Entries skipped: 1 binary"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("5 hits"), "summary in stderr: {stderr}");
    Ok(())
}

#[test]
fn in_and_lang_narrow_the_sources() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write(dir.path(), &builder())?;
    p4kx_testkit::p4kx()?
        .arg("grep")
        .arg(&archive)
        .arg("gladius")
        .args(["--in", "loc", "--lang", "en"])
        .assert()
        .code(0)
        .stdout(
            predicate::str::contains("vehicle_NameAEGS_Gladius")
                .and(predicate::str::contains("RECORD PATH").not())
                .and(predicate::str::contains("ENTRY:LINE").not()),
        );
    p4kx_testkit::p4kx()?
        .arg("grep")
        .arg(&archive)
        .arg("fighter")
        .args(["--in", "records,entries", "--filter", "*/ships/*"])
        .assert()
        .code(0)
        .stdout(
            predicate::str::contains("Gear.AEGS_Gladius")
                .and(predicate::str::contains("Data\\Scripts").not()),
        );
    Ok(())
}

#[test]
fn json_has_the_hits_and_the_counts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write(dir.path(), &builder())?;
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .arg("grep")
        .arg(&archive)
        .arg("Gladi")
        .args(["--limit", "1"])
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["text"], "Gladi", "text");
    assert_eq!(json["records"].as_array().map(Vec::len), Some(1), "limited");
    assert_eq!(json["records"][0]["field"], "name", "field");
    assert_eq!(json["records"][0]["type"], "Gear", "type");
    assert_eq!(
        json["records"][0]["match"],
        serde_json::json!([5, 10]),
        "match"
    );
    assert_eq!(
        json["records"][0]["guid"], "cbee34fd-6441-4000-b99f-924f7f26e19f",
        "guid"
    );
    assert_eq!(json["loc"][0]["language"], "en", "language");
    assert_eq!(json["entries"][0]["line"], 1, "line");
    assert_eq!(
        json["more"],
        serde_json::json!(["records", "entries"]),
        "more"
    );
    assert_eq!(json["stats"]["entries_binary"], 1, "binary");
    assert!(json["elapsed_ms"].is_u64(), "elapsed: {json}");
    Ok(())
}

#[test]
fn an_empty_text_and_json_with_a_table_are_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write(dir.path(), &builder())?;
    p4kx_testkit::p4kx()?
        .arg("grep")
        .arg(&archive)
        .arg("")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("empty"));
    p4kx_testkit::p4kx()?
        .arg("--json")
        .arg("grep")
        .arg(&archive)
        .arg("x")
        .args(["--format", "table"])
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn encrypted_entries_without_a_key_are_skipped_with_a_hint() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write(dir.path(), &builder())?;
    keyless(dir.path())?
        .arg("grep")
        .arg(&archive)
        .arg("gladius")
        .args(["--in", "entries"])
        .assert()
        .code(0)
        .stdout(
            predicate::str::contains("ship.xml:2")
                .and(predicate::str::contains("secret.xml").not()),
        )
        .stderr(
            predicate::str::contains("1 encrypted without a matching Archive key")
                .and(predicate::str::contains("p4kx key set")),
        );
    Ok(())
}

#[test]
fn an_encrypted_dataforge_without_a_key_exits_with_the_key_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::zstd("Data\\Game2.dcb", forge()).encrypted()),
    )?;
    keyless(dir.path())?
        .arg("grep")
        .arg(&archive)
        .arg("gladius")
        .args(["--in", "records"])
        .assert()
        .code(7)
        .stderr(predicate::str::contains("hint:"));
    Ok(())
}

#[test]
fn text_from_the_archive_cannot_carry_control_characters_to_the_terminal() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(
            "a\\\u{1b}[31mred\u{202e}.txt",
            "first\nthe \u{c}gladius\u{9b}2J\u{202e}\n",
        )),
    )?;
    let output = p4kx_testkit::p4kx()?
        .arg("grep")
        .arg(&archive)
        .arg("gladius")
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        !stdout.chars().any(|c| c.is_control() && c != '\n'),
        "no control character in: {stdout:?}"
    );
    assert!(!stdout.contains('\u{202e}'), "no override in: {stdout:?}");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_real_archive_is_searched_in_records_and_localisation() -> TestResult {
    let archive = real_archive_path()?;
    let output = keyless(tempfile::tempdir()?.path())?
        .arg("grep")
        .arg(&archive)
        .arg("gladius")
        .args(["--in", "records,loc", "--lang", "en", "--limit", "3"])
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("RECORD PATH"), "Records: {stdout}");
    assert!(stdout.contains("LANGUAGE"), "localisation: {stdout}");
    Ok(())
}
