// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx catalog` and the Archive key: the catalog and the run say when encrypted Entries stayed
//! unread, once.

use crate::catalog_support;

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Output;

use catalog_support::{EXPORTED_AT, write_archive, write_encrypted_vehicle_archive};
use p4kx_testkit::{other_test_key, test_key};

type TestResult = Result<(), Box<dyn Error>>;

const NOTE: &str = "note: ";

fn keyless_command(dir: &Path) -> Result<assert_cmd::Command, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .env("SOURCE_DATE_EPOCH", EXPORTED_AT)
        .env_remove("P4KX_ARCHIVE_KEY")
        .env("P4KX_KEY_FILE", dir.join("profile").join("archive.key"));
    Ok(command)
}

fn catalog(
    dir: &Path,
    archive: &Path,
    key: Option<&str>,
    args: &[&str],
) -> Result<(Output, serde_json::Value), Box<dyn Error>> {
    let out = dir.join("catalog.json");
    let mut command = keyless_command(dir)?;
    command
        .arg("catalog")
        .arg(archive)
        .arg("--out")
        .arg(&out)
        .args(args);
    if let Some(key) = key {
        command.arg("--key").arg(key);
    }
    let output = command.output()?;
    let catalog =
        serde_json::from_slice(&fs::read(&out).map_err(|error| format!("{error}: {output:?}"))?)?;
    Ok((output, catalog))
}

fn notes(output: &Output) -> Result<Vec<String>, Box<dyn Error>> {
    Ok(String::from_utf8(output.stderr.clone())?
        .lines()
        .filter(|line| line.starts_with(NOTE))
        .map(str::to_owned)
        .collect())
}

#[test]
fn a_catalog_without_the_key_says_so_in_the_file_and_once_on_stderr() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_encrypted_vehicle_archive(dir.path())?;

    let (output, catalog) = catalog(dir.path(), &archive, None, &["--diagnostic-partial"])?;

    assert_eq!(
        output.status.code(),
        Some(1),
        "an incomplete export, as before"
    );
    assert_eq!(catalog["source"]["archiveKey"], "missing", "file");
    assert_eq!(catalog["source"]["encryptedEntriesUnread"], 1, "count");
    let notes = notes(&output)?;
    assert_eq!(notes.len(), 1, "one note: {notes:?}");
    assert!(
        notes[0].starts_with("note: no Archive key: 1 encrypted Entry was not read (the first is ")
            && notes[0].contains("ACAS_Hauler.xml")
            && notes[0].contains("the result is incomplete")
            && notes[0].contains("--key <hex>")
            && notes[0].contains("p4kx key set"),
        "{}",
        notes[0]
    );
    Ok(())
}

#[test]
fn a_catalog_with_the_key_has_no_note_and_states_the_key() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_encrypted_vehicle_archive(dir.path())?;

    let (output, catalog) = catalog(dir.path(), &archive, Some(&test_key().to_hex()), &[])?;

    assert_eq!(output.status.code(), Some(0), "exit code");
    assert_eq!(catalog["source"]["archiveKey"], "given", "file");
    assert_eq!(catalog["source"]["encryptedEntriesUnread"], 0, "count");
    assert!(notes(&output)?.is_empty(), "{output:?}");
    Ok(())
}

#[test]
fn a_catalog_with_a_wrong_key_names_the_wrong_key() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_encrypted_vehicle_archive(dir.path())?;

    let (output, catalog) = catalog(
        dir.path(),
        &archive,
        Some(&other_test_key().to_hex()),
        &["--diagnostic-partial"],
    )?;

    assert_eq!(
        output.status.code(),
        Some(1),
        "an incomplete export, as before"
    );
    assert_eq!(catalog["source"]["archiveKey"], "wrong", "file");
    assert_eq!(catalog["source"]["encryptedEntriesUnread"], 1, "count");
    let notes = notes(&output)?;
    assert_eq!(notes.len(), 1, "one note: {notes:?}");
    assert!(
        notes[0].starts_with("note: the Archive key does not fit: 1 encrypted Entry was not read"),
        "{}",
        notes[0]
    );
    Ok(())
}

#[test]
fn the_json_summary_carries_the_key_state_and_the_count() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_encrypted_vehicle_archive(dir.path())?;

    let (output, _) = catalog(
        dir.path(),
        &archive,
        None,
        &["--json", "--diagnostic-partial"],
    )?;

    let summary: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(summary["archive_key"], "missing", "state");
    assert_eq!(summary["encrypted_entries_unread"], 1, "count");
    Ok(())
}

#[test]
fn a_command_that_asks_for_no_encrypted_entry_prints_no_note() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let output = keyless_command(dir.path())?
        .arg("list")
        .arg(&archive)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "exit code");
    assert!(notes(&output)?.is_empty(), "{output:?}");
    Ok(())
}
