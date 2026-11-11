// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx dcb info` end to end, over synthetic DataForge databases of versions 6 and 8 and, with
//! `--ignored`, over the real Archive `P4KX_DATA_P4K` names.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType,
    real_archive_path,
};

type TestResult = Result<(), Box<dyn Error>>;

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Large"])
        .enumeration("Manufacturer", ["AEGS", "ANVL", "RSI"])
        .structure(TestStruct::new("Item").field("size", TestType::Enum("Size".to_owned())))
        .structure(
            TestStruct::new("Ship")
                .parent("Item")
                .field("crew", TestType::Int32),
        )
        .structure(TestStruct::new("Weapon").parent("Item"))
        .record(TestRecord::new(
            0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f,
            "Ship",
            "Ship.Avenger",
            "libs/foundry/records/ships/avenger.xml",
        ))
        .record(TestRecord::new(
            0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d,
            "Weapon",
            "Weapon.Gun",
            "libs/foundry/records/weapons/gun.xml",
        ))
}

fn write_archive(dir: &Path, entries: Vec<TestEntry>) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

fn run(archive: &Path, json: bool, code: i32) -> Result<(String, String), Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let output = command.arg("dcb").arg("info").arg(archive).output()?;
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
            .replace("[DIR]\\\\", "[DIR]/")
            .replace("[DIR]\\", "[DIR]/"))
    };
    Ok((redact(output.stdout)?, redact(output.stderr)?))
}

#[test]
fn dcb_info_prints_the_version_and_the_counts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(
        dir.path(),
        vec![TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted()],
    )?;

    let (stdout, stderr) = run(&archive, false, 0)?;

    insta::assert_snapshot!(stdout);
    assert!(stderr.is_empty(), "no warnings: {stderr}");
    Ok(())
}

#[test]
fn dcb_info_prints_one_line_of_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(
        dir.path(),
        vec![TestEntry::deflated("Data\\Game2.dcb", database().build())],
    )?;

    let (stdout, _) = run(&archive, true, 0)?;

    assert_eq!(stdout.lines().count(), 1, "one line: {stdout}");
    let mut value: serde_json::Value = serde_json::from_str(&stdout)?;
    let fields = value.as_object_mut().ok_or("DataForge summary object")?;
    fields.remove("fieldOrigins");
    fields.remove("provenance");
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[test]
fn dcb_info_reads_version_8() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(
        dir.path(),
        vec![TestEntry::zstd("Data\\Game2.dcb", database().version(8).build()).encrypted()],
    )?;

    let (stdout, stderr) = run(&archive, false, 0)?;
    let (json, _) = run(&archive, true, 0)?;

    insta::assert_snapshot!(stdout);
    assert!(stderr.is_empty(), "no warnings: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&json)?;
    assert_eq!(value["version"], 8, "JSON version: {json}");
    assert_eq!(value["record_count"], 2, "JSON Records: {json}");
    assert_eq!(value["struct_count"], 3, "JSON structs: {json}");
    assert_eq!(value["enum_count"], 2, "JSON enums: {json}");
    Ok(())
}

#[test]
fn dcb_info_falls_back_to_game_dcb() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(
        dir.path(),
        vec![TestEntry::stored("Data\\Game.dcb", database().build())],
    )?;

    let (stdout, _) = run(&archive, false, 0)?;

    assert!(
        stdout.contains("Entry:           Data\\Game.dcb\n"),
        "Game.dcb is read: {stdout}"
    );
    Ok(())
}

#[test]
fn dcb_info_of_an_unknown_version_exits_with_code_5() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(
        dir.path(),
        vec![TestEntry::stored(
            "Data\\Game2.dcb",
            database().version(7).build(),
        )],
    )?;

    let (stdout, stderr) = run(&archive, false, 5)?;

    assert!(stdout.is_empty(), "nothing is printed: {stdout}");
    insta::assert_snapshot!(stderr);
    Ok(())
}

#[test]
fn dcb_info_of_version_5_and_4_exits_with_code_5_as_unsupported() -> TestResult {
    for version in [5, 4] {
        let dir = tempfile::tempdir()?;
        let archive = write_archive(
            dir.path(),
            vec![TestEntry::stored(
                "Data\\Game2.dcb",
                database().version(version).build(),
            )],
        )?;

        let (stdout, stderr) = run(&archive, false, 5)?;

        assert!(stdout.is_empty(), "nothing is printed: {stdout}");
        assert_eq!(
            stderr.trim_end(),
            format!(
                "error: DataForge version {version} is not supported; this version of P4KX reads versions 6 and 8"
            ),
            "version {version}"
        );
    }
    Ok(())
}

#[test]
fn dcb_info_of_a_truncated_database_exits_with_code_5() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut bytes = database().build();
    bytes.truncate(200);
    let archive = write_archive(
        dir.path(),
        vec![TestEntry::stored("Data\\Game2.dcb", bytes)],
    )?;

    let (_, stderr) = run(&archive, false, 5)?;

    insta::assert_snapshot!(stderr);
    Ok(())
}

#[test]
fn dcb_info_of_an_archive_without_dataforge_exits_with_code_5() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(
        dir.path(),
        vec![TestEntry::stored("Data\\readme.txt", b"hello".to_vec())],
    )?;

    let (_, stderr) = run(&archive, false, 5)?;

    insta::assert_snapshot!(stderr);
    Ok(())
}

#[test]
fn dcb_info_of_a_corrupt_entry_exits_with_code_3() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(
        dir.path(),
        vec![
            TestEntry::deflated("Data\\Game2.dcb", database().build()).with_payload(vec![0xFF; 8]),
        ],
    )?;

    let (_, stderr) = run(&archive, false, 3)?;

    assert!(
        stderr.contains("cannot read the DataForge Entry Data\\Game2.dcb"),
        "the Entry is named: {stderr}"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn dcb_info_reads_the_dataforge_of_a_real_archive() -> TestResult {
    let archive = real_archive_path()?;

    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .arg("dcb")
        .arg("info")
        .arg(&archive)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert!(
        matches!(value["version"].as_u64(), Some(6 | 8)),
        "a supported version: {value}"
    );
    for count in ["record_count", "struct_count", "enum_count"] {
        assert!(
            value[count].as_u64().is_some_and(|count| count > 0),
            "{count} is positive: {value}"
        );
    }
    Ok(())
}

#[path = "dcb_info_origins.rs"]
mod origins;
