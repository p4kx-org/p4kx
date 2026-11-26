// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Archive key on the command line: `--key`, `P4KX_ARCHIVE_KEY`, the stored key and the
//! `key` commands.

use std::error::Error;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, other_test_key, test_key};
use predicates::prelude::{PredicateBooleanExt, predicate};

mod json;

type TestResult = Result<(), Box<dyn Error>>;

const SECRET: &[u8] = b"<secret data of the Archive/>";

fn archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\plain.xml", b"<plain/>".to_vec()))
        .entry(TestEntry::deflated("Data\\secret.xml", SECRET.to_vec()).encrypted())
        .write_to(&path)?;
    Ok(path)
}

fn plain_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Plain.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\plain.xml", b"<plain/>".to_vec()))
        .write_to(&path)?;
    Ok(path)
}

fn keyless(dir: &Path) -> Result<Command, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .env_remove("P4KX_ARCHIVE_KEY")
        .env("P4KX_KEY_FILE", dir.join("profile").join("archive.key"));
    Ok(command)
}

fn extract(command: &mut Command, archive: &Path, out: &Path) {
    command
        .arg("extract")
        .arg(archive)
        .arg("Data\\secret.xml")
        .arg("--out")
        .arg(out);
}

#[test]
fn an_archive_without_a_key_still_lists_searches_and_reads_unencrypted_entries() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    keyless(dir.path())?
        .arg("list")
        .arg(&archive)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("Data\\secret.xml"));
    keyless(dir.path())?
        .arg("extract")
        .arg(&archive)
        .arg("Data\\plain.xml")
        .arg("--out")
        .arg(dir.path().join("out"))
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read(dir.path().join("out").join("Data").join("plain.xml"))?,
        b"<plain/>",
        "unencrypted Entry"
    );
    Ok(())
}

#[test]
fn an_encrypted_entry_without_a_key_exits_with_7_and_says_how_to_give_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("out");
    let mut command = keyless(dir.path())?;
    extract(&mut command, &archive, &out);

    command
        .assert()
        .code(7)
        .stderr(predicate::str::contains("encrypted"))
        .stderr(predicate::str::contains("--key"))
        .stderr(predicate::str::contains("P4KX_ARCHIVE_KEY"))
        .stderr(predicate::str::contains("p4kx key set"));
    assert!(
        !out.join("Data").join("secret.xml").exists(),
        "nothing written"
    );
    Ok(())
}

#[test]
fn the_key_flag_the_variable_and_the_stored_key_each_read_the_entry() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let hex = test_key().to_hex();
    let read = |out: &str| std::fs::read(dir.path().join(out).join("Data").join("secret.xml"));

    let mut flag = keyless(dir.path())?;
    flag.arg("--key").arg(&hex);
    extract(&mut flag, &archive, &dir.path().join("flag"));
    flag.assert().code(0);
    assert_eq!(read("flag")?, SECRET, "flag");

    let mut variable = keyless(dir.path())?;
    variable.env("P4KX_ARCHIVE_KEY", &hex);
    extract(&mut variable, &archive, &dir.path().join("variable"));
    variable.assert().code(0);
    assert_eq!(read("variable")?, SECRET, "variable");

    keyless(dir.path())?
        .args(["key", "set", &hex, "--archive"])
        .arg(&archive)
        .assert()
        .code(0);
    let mut stored = keyless(dir.path())?;
    extract(&mut stored, &archive, &dir.path().join("stored"));
    stored.assert().code(0);
    assert_eq!(read("stored")?, SECRET, "stored");
    Ok(())
}

#[test]
fn a_wrong_key_exits_with_7_and_is_never_echoed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let wrong = other_test_key().to_hex();
    let mut command = keyless(dir.path())?;
    command.arg("--key").arg(&wrong);
    extract(&mut command, &archive, &dir.path().join("out"));

    let output = command.output()?;

    assert_eq!(output.status.code(), Some(7), "{output:?}");
    let shown = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!shown.contains(&wrong), "the key is not repeated");
    assert!(shown.contains("does not"), "{shown}");
    Ok(())
}

#[test]
fn a_mistyped_key_is_invalid_input_and_is_never_echoed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let mistyped = format!("{}zz", &test_key().to_hex()[2..]);

    let output = keyless(dir.path())?
        .arg("--key")
        .arg(&mistyped)
        .arg("list")
        .arg(&archive)
        .output()?;

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains(&mistyped[..10]),
        "the key is not repeated"
    );
    Ok(())
}

#[test]
fn the_flag_beats_the_variable_and_the_variable_beats_the_stored_key() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let good = test_key().to_hex();
    let wrong = other_test_key().to_hex();
    keyless(dir.path())?
        .args(["key", "set", &wrong, "--archive"])
        .arg(plain_archive(dir.path())?)
        .assert()
        .code(0);

    let mut stored_wins = keyless(dir.path())?;
    extract(&mut stored_wins, &archive, &dir.path().join("a"));
    stored_wins.assert().code(7);

    let mut variable = keyless(dir.path())?;
    variable.env("P4KX_ARCHIVE_KEY", &good);
    extract(&mut variable, &archive, &dir.path().join("b"));
    variable.assert().code(0);

    let mut flag = keyless(dir.path())?;
    flag.env("P4KX_ARCHIVE_KEY", &wrong).arg("--key").arg(&good);
    extract(&mut flag, &archive, &dir.path().join("c"));
    flag.assert().code(0);
    Ok(())
}

#[test]
fn key_set_reads_stdin_checks_the_key_and_stores_it_privately() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let hex = test_key().to_hex();
    let file = dir.path().join("profile").join("archive.key");

    keyless(dir.path())?
        .args(["key", "set", "--archive"])
        .arg(&archive)
        .write_stdin(format!("{hex}\n"))
        .assert()
        .code(0)
        .stdout(predicate::str::contains("it decrypts an Entry"))
        .stdout(predicate::str::contains(&hex).not());

    assert_eq!(std::fs::read_to_string(&file)?.trim(), hex, "stored");
    Ok(())
}

#[test]
fn key_set_refuses_a_wrong_key_and_stores_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    keyless(dir.path())?
        .args(["key", "set", &other_test_key().to_hex(), "--archive"])
        .arg(&archive)
        .assert()
        .code(7);

    assert!(
        !dir.path().join("profile").join("archive.key").exists(),
        "nothing stored"
    );
    Ok(())
}

#[test]
fn key_set_refuses_text_that_is_no_key() -> TestResult {
    let dir = tempfile::tempdir()?;

    keyless(dir.path())?
        .args(["key", "set", "not-a-key"])
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn key_show_masks_the_key_and_names_its_source() -> TestResult {
    let dir = tempfile::tempdir()?;
    let hex = test_key().to_hex();
    keyless(dir.path())?
        .args(["key", "set", &hex, "--archive"])
        .arg(plain_archive(dir.path())?)
        .assert()
        .code(0);

    let output = keyless(dir.path())?.args(["key", "show"]).output()?;
    let text = String::from_utf8(output.stdout)?;
    assert!(text.contains("Stored: ****"), "{text}");
    assert!(text.contains("(profile)"), "{text}");
    assert!(!text.contains(&hex), "{text}");

    let json = keyless(dir.path())?
        .args(["--json", "key", "show"])
        .output()?;
    let value: serde_json::Value = serde_json::from_slice(&json.stdout)?;
    assert_eq!(value["in_use"]["source"], "profile", "{value}");
    assert_eq!(value["originApplicability"], "notApplicable", "{value}");
    assert!(
        !String::from_utf8_lossy(&json.stdout).contains(&hex),
        "masked"
    );
    Ok(())
}

#[test]
fn key_remove_deletes_the_stored_key() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    keyless(dir.path())?
        .args(["key", "set", &test_key().to_hex(), "--archive"])
        .arg(plain_archive(dir.path())?)
        .assert()
        .code(0);

    keyless(dir.path())?
        .args(["key", "remove"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("removed"));
    keyless(dir.path())?
        .args(["key", "remove"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("no Archive key was stored"));

    let mut command = keyless(dir.path())?;
    extract(&mut command, &archive, &dir.path().join("out"));
    command.assert().code(7);
    Ok(())
}

#[test]
fn help_names_the_key_option_and_the_variable() -> TestResult {
    keyless(Path::new("."))?
        .arg("--help")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("--key <HEX>"))
        .stdout(predicate::str::contains("P4KX_ARCHIVE_KEY"))
        .stdout(predicate::str::contains(
            "7  the Archive key is missing or wrong",
        ));
    Ok(())
}

#[test]
fn a_command_that_opens_an_encrypted_container_exits_with_7_without_a_key() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let room = TestContainer::new().entry(TestEntry::stored("room.xml", b"<Room/>".to_vec()));
    ArchiveBuilder::new()
        .entry(TestEntry::deflated("Data\\room.socpak", room.build()).encrypted())
        .write_to(&path)?;

    keyless(dir.path())?
        .args(["socpak", "list"])
        .arg(&path)
        .arg("Data\\room.socpak")
        .assert()
        .code(7)
        .stderr(predicate::str::contains("p4kx key set"));
    keyless(dir.path())?
        .args(["--key", &test_key().to_hex(), "socpak", "list"])
        .arg(&path)
        .arg("Data\\room.socpak")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("room.xml"));
    Ok(())
}

#[test]
fn comparing_an_encrypted_entry_line_by_line_without_a_key_exits_with_7() -> TestResult {
    let dir = tempfile::tempdir()?;
    let old = archive(dir.path())?;
    let new = dir.path().join("New.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\plain.xml", b"<plain/>".to_vec()))
        .entry(TestEntry::deflated("Data\\secret.xml", b"<other/>".to_vec()).encrypted())
        .write_to(&new)?;

    keyless(dir.path())?
        .arg("compare")
        .arg(&old)
        .arg(&new)
        .args(["--diff", "Data\\secret.xml"])
        .assert()
        .code(7)
        .stderr(predicate::str::contains("P4KX_ARCHIVE_KEY"));
    Ok(())
}

#[test]
fn verifying_an_encrypted_entry_without_a_key_exits_with_7() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;

    keyless(dir.path())?
        .arg("verify")
        .arg(&archive)
        .assert()
        .code(7);
    keyless(dir.path())?
        .arg("verify")
        .arg(&archive)
        .args(["--filter", "plain"])
        .assert()
        .code(0);
    Ok(())
}
