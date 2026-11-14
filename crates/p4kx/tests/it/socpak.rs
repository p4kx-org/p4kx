// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx socpak list|extract` end to end, over a synthetic Archive with nested socpak containers.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};
use predicates::prelude::predicate;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const SHIP: &str = "Data\\ObjectContainers\\Ships\\AEGS\\ship_interior.socpak";
const ROOM: &str = "ship_interior\\modules\\cockpit.socpak";

fn cockpit() -> TestContainer {
    TestContainer::new()
        .entry(TestEntry::deflated(
            "cockpit\\entdata\\1001.entxml",
            "<Entity Name=\"seat\"/>",
        ))
        .entry(TestEntry::stored(
            "cockpit\\cockpit.xml",
            "<ObjectContainer/>",
        ))
}

fn ship() -> TestContainer {
    TestContainer::new()
        .entry(TestEntry::stored(
            "ship_interior\\ship_interior.xml",
            "<ObjectContainer Name=\"ship\"/>",
        ))
        .entry(TestEntry::deflated(
            "ship_interior/entdata/2002.entxml",
            "<Entity Name=\"door\"/>",
        ))
        .container(ROOM, &cockpit())
}

fn write_archive(folder: &Path, ship: &TestContainer) -> Result<PathBuf, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\readme.txt", "hello"))
        .entry(TestEntry::stored(SHIP, ship.build()))
        .entry(TestEntry::stored("Data\\broken.socpak", "no container"))
        .write_to(&path)?;
    Ok(path)
}

fn p4kx() -> Result<Command, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?)
}

#[test]
fn list_prints_the_entries_of_a_container() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &ship())?;

    let output = p4kx()?
        .args(["socpak", "list"])
        .arg(&archive)
        .arg("data/objectcontainers/ships/aegs/SHIP_INTERIOR.socpak")
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    insta::assert_snapshot!(String::from_utf8(output.stdout)?);
    Ok(())
}

#[test]
fn list_follows_containers_inside_containers() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &ship())?;

    let output = p4kx()?
        .args(["socpak", "list"])
        .arg(&archive)
        .args([SHIP, ROOM])
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    insta::assert_snapshot!(String::from_utf8(output.stdout)?);
    Ok(())
}

#[test]
fn list_as_json_names_the_archive_and_the_containers() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &ship())?;

    let output = p4kx()?
        .args(["--json", "socpak", "list"])
        .arg(&archive)
        .args([SHIP, ROOM])
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert_eq!(stdout.lines().count(), 1, "one line of JSON: {stdout}");
    let mut value: Value = serde_json::from_str(&stdout)?;
    if let Some(Value::String(path)) = value.get_mut("archive") {
        *path = path
            .replace(&dir.path().display().to_string(), "[DIR]")
            .replace('\\', "/");
    }
    crate::entry_listing_origins::remove_checked_origins_for_snapshot(&mut value)?;
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[test]
fn extract_writes_every_entry_of_the_container() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &ship())?;
    let out = dir.path().join("out");

    p4kx()?
        .args(["socpak", "extract"])
        .arg(&archive)
        .arg(SHIP)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("extracted 3, skipped 0, failed 0"));

    let folder = out.join("ship_interior");
    assert_eq!(
        fs::read_to_string(folder.join("ship_interior.xml"))?,
        "<ObjectContainer Name=\"ship\"/>",
        "a stored Entry"
    );
    assert_eq!(
        fs::read_to_string(folder.join("entdata").join("2002.entxml"))?,
        "<Entity Name=\"door\"/>",
        "a deflated Entry"
    );
    assert_eq!(
        fs::read(folder.join("modules").join("cockpit.socpak"))?,
        cockpit().build(),
        "a nested container stays one file"
    );
    Ok(())
}

#[test]
fn extract_takes_a_filter_and_extracts_from_nested_containers() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &ship())?;
    let out = dir.path().join("out");

    p4kx()?
        .args(["socpak", "extract"])
        .arg(&archive)
        .args([SHIP, ROOM, "--filter", "*.entxml", "--out"])
        .arg(&out)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("extracted 1, skipped 0"));

    assert_eq!(
        fs::read_to_string(out.join("cockpit").join("entdata").join("1001.entxml"))?,
        "<Entity Name=\"seat\"/>",
        "the matching Entry of the inner container"
    );
    assert!(
        !out.join("cockpit").join("cockpit.xml").exists(),
        "the Entry the Filter does not match"
    );
    Ok(())
}

#[test]
fn extract_applies_the_conflict_policy_with_the_crc_32_of_containers() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &ship())?;
    let out = dir.path().join("out");
    let run = |conflict: &str| -> Result<String, Box<dyn Error>> {
        let output = p4kx()?
            .args(["socpak", "extract"])
            .arg(&archive)
            .args([SHIP, ROOM, "--conflict", conflict, "--out"])
            .arg(&out)
            .output()?;
        assert_eq!(output.status.code(), Some(0), "success: {output:?}");
        Ok(String::from_utf8(output.stdout)?)
    };
    run("skip")?;
    let changed = out.join("cockpit").join("cockpit.xml");
    fs::write(&changed, "<Edited/>>>>>>>>>>")?;

    let skipped = run("skip")?;
    let if_changed = run("if-changed")?;

    assert!(
        skipped.contains("extracted 0, skipped 2"),
        "skip keeps both files: {skipped}"
    );
    assert!(
        if_changed.contains("extracted 1, skipped 1")
            && if_changed.contains("skipped cockpit\\entdata\\1001.entxml: unchanged"),
        "if-changed rewrites only the edited file: {if_changed}"
    );
    assert_eq!(
        fs::read_to_string(&changed)?,
        "<ObjectContainer/>",
        "the edited file is restored"
    );
    Ok(())
}

#[test]
fn extract_refuses_entry_paths_that_leave_the_target() -> TestResult {
    let dir = tempfile::tempdir()?;
    let evil = TestContainer::new()
        .entry(TestEntry::stored("..\\escaped.txt", "outside"))
        .entry(TestEntry::stored("C:\\absolute.txt", "outside"))
        .entry(TestEntry::stored("inside.txt", "inside"));
    let archive = write_archive(dir.path(), &evil)?;
    let out = dir.path().join("target").join("out");

    p4kx()?
        .args(["socpak", "extract"])
        .arg(&archive)
        .arg(SHIP)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("extracted 1, skipped 0, failed 2"))
        .stderr(predicate::str::contains("failed ..\\escaped.txt"));

    assert!(
        !dir.path().join("target").join("escaped.txt").exists(),
        "nothing is written outside the target"
    );
    assert_eq!(fs::read_to_string(out.join("inside.txt"))?, "inside");
    Ok(())
}

#[test]
fn a_corrupt_container_exits_with_code_3_naming_the_entry() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &ship())?;

    p4kx()?
        .args(["socpak", "list"])
        .arg(&archive)
        .arg("Data\\broken.socpak")
        .assert()
        .code(3)
        .stderr(predicate::str::contains(
            "Entry Data\\broken.socpak cannot be opened as a container",
        ));
    Ok(())
}

#[test]
fn an_unknown_container_or_a_filter_without_match_exits_with_code_2() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &ship())?;

    p4kx()?
        .args(["socpak", "list"])
        .arg(&archive)
        .args([SHIP, "ship_interior\\missing.socpak"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "no Entry named ship_interior\\missing.socpak",
        ));
    p4kx()?
        .args(["socpak", "extract"])
        .arg(&archive)
        .args([SHIP, "--filter", "*.dds", "--out"])
        .arg(dir.path().join("out"))
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no Entry matches *.dds"));
    Ok(())
}

#[test]
fn extract_as_json_prints_the_report_of_p4kx_extract() -> TestResult {
    let dir = tempfile::tempdir()?;
    let container = ship().entry(TestEntry::stored("..\\escaped.txt", "outside"));
    let archive = write_archive(dir.path(), &container)?;
    let out = dir.path().join("out");

    let output = p4kx()?
        .args(["--json", "socpak", "extract"])
        .arg(&archive)
        .arg(SHIP)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(1), "one Entry fails: {output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert_eq!(stdout.lines().count(), 1, "one line of JSON: {stdout}");
    let mut value: Value = serde_json::from_str(&stdout)?;
    if let Some(Value::String(target)) = value.get_mut("target") {
        *target = target
            .replace(&dir.path().display().to_string(), "[DIR]")
            .replace('\\', "/");
    }
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}
