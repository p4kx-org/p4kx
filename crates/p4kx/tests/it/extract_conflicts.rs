// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx extract` into a pre-filled target under each Conflict policy, and cancelled runs.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

struct Prefilled {
    dir: tempfile::TempDir,
    archive: PathBuf,
    out: PathBuf,
}

fn prefilled() -> Result<Prefilled, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\same.txt", b"same".to_vec()))
        .entry(TestEntry::deflated("Data\\edited.txt", b"new!".to_vec()))
        .entry(TestEntry::zstd("Data\\resized.txt", b"longer".to_vec()).encrypted())
        .entry(TestEntry::stored("Data\\fresh.txt", b"fresh".to_vec()))
        .write_to(&archive)?;
    let out = dir.path().join("out");
    fs::create_dir_all(out.join("Data"))?;
    fs::write(out.join("Data/same.txt"), "same")?;
    fs::write(out.join("Data/edited.txt"), "old!")?;
    fs::write(out.join("Data/resized.txt"), "short")?;
    Ok(Prefilled { dir, archive, out })
}

fn extract_all(fixture: &Prefilled) -> Result<Command, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .arg("extract")
        .arg(&fixture.archive)
        .args(["--filter", "Data\\"])
        .arg("--out")
        .arg(&fixture.out);
    Ok(command)
}

fn contents(out: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    ["same.txt", "edited.txt", "resized.txt", "fresh.txt"]
        .iter()
        .map(|name| Ok(fs::read_to_string(out.join("Data").join(name))?))
        .collect()
}

#[test]
fn skip_is_the_default_and_keeps_every_existing_file() -> TestResult {
    let fixture = prefilled()?;

    extract_all(&fixture)?
        .assert()
        .code(0)
        .stdout(predicate::str::contains(
            "skipped Data\\same.txt: already exists",
        ))
        .stdout(predicate::str::contains(
            "skipped Data\\edited.txt: already exists",
        ))
        .stdout(predicate::str::contains(
            "extracted 1, skipped 3, failed 0, cancelled 0",
        ));

    assert_eq!(
        contents(&fixture.out)?,
        ["same", "old!", "short", "fresh"],
        "existing files are unchanged"
    );
    Ok(())
}

#[test]
fn conflict_skip_keeps_every_existing_file() -> TestResult {
    let fixture = prefilled()?;

    extract_all(&fixture)?
        .args(["--conflict", "skip"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains(
            "extracted 1, skipped 3, failed 0, cancelled 0",
        ));

    assert_eq!(
        contents(&fixture.out)?,
        ["same", "old!", "short", "fresh"],
        "existing files are unchanged"
    );
    Ok(())
}

#[test]
fn conflict_overwrite_replaces_every_existing_file() -> TestResult {
    let fixture = prefilled()?;

    extract_all(&fixture)?
        .args(["--conflict", "overwrite"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains(
            "extracted 4, skipped 0, failed 0, cancelled 0",
        ));

    assert_eq!(
        contents(&fixture.out)?,
        ["same", "new!", "longer", "fresh"],
        "every file holds the Entry data"
    );
    Ok(())
}

#[test]
fn conflict_if_changed_replaces_only_changed_files() -> TestResult {
    let fixture = prefilled()?;

    extract_all(&fixture)?
        .args(["--conflict", "if-changed"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains(
            "skipped Data\\same.txt: unchanged",
        ))
        .stdout(predicate::str::contains(
            "extracted 3, skipped 1, failed 0, cancelled 0",
        ));

    assert_eq!(
        contents(&fixture.out)?,
        ["same", "new!", "longer", "fresh"],
        "every file holds the Entry data"
    );
    Ok(())
}

#[test]
fn an_unknown_conflict_policy_is_invalid_input_with_exit_code_2() -> TestResult {
    let fixture = prefilled()?;

    extract_all(&fixture)?
        .args(["--conflict", "merge"])
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn a_cancelled_overwrite_keeps_the_files_it_did_not_reach_with_exit_code_1() -> TestResult {
    let fixture = prefilled()?;

    extract_all(&fixture)?
        .env("RAYON_NUM_THREADS", "1")
        .args(["--conflict", "overwrite", "--cancel-after", "1"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("cancelled Data\\edited.txt"))
        .stdout(predicate::str::contains("cancelled Data\\resized.txt"))
        .stdout(predicate::str::contains("cancelled Data\\fresh.txt"))
        .stdout(predicate::str::contains(
            "extracted 1, skipped 0, failed 0, cancelled 3",
        ));

    let mut files: Vec<_> = fs::read_dir(fixture.out.join("Data"))?
        .map(|item| Ok(item?.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, std::io::Error>>()?;
    files.sort();
    assert_eq!(
        files,
        [
            "edited.txt",
            "resized.txt",
            "same.txt",
            "same.txt.p4kx-origin.jsonl",
        ],
        "no partial or temporary file is left"
    );
    assert_eq!(
        fs::read_to_string(fixture.out.join("Data/edited.txt"))?,
        "old!",
        "a cancelled overwrite keeps the existing file"
    );
    assert!(
        fixture.dir.path().join("Data.p4k").exists(),
        "the Archive is untouched"
    );
    Ok(())
}
#[test]
fn a_cancelled_run_lists_the_cancelled_entries_in_json() -> TestResult {
    let fixture = prefilled()?;
    fs::remove_dir_all(&fixture.out)?;

    let output = extract_all(&fixture)?
        .env("RAYON_NUM_THREADS", "1")
        .args(["--cancel-after", "1", "--json"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let report: serde_json::Value = serde_json::from_slice(&output)?;
    assert_eq!(
        report["cancelled"],
        serde_json::json!(["Data\\edited.txt", "Data\\resized.txt", "Data\\fresh.txt"]),
        "the cancelled Entries in order"
    );
    Ok(())
}
