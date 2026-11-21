// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx verify`, `p4kx extract --verify` and `p4kx socpak verify` end to end.

use std::error::Error;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};
use predicates::prelude::predicate;
use serde_json::Value;
#[path = "verify_origins.rs"]
mod origins;

type TestResult = Result<(), Box<dyn Error>>;

const DATA: &[u8] = b"verified payload for the command line tests";
const BLOCKS: &[u8] = b"0123456789abcdef0123456789abcdef";

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let archive = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\good.bin", DATA))
        .entry(TestEntry::deflated("Data\\bad.bin", DATA).with_crc32c(0x1234_5678))
        .entry(TestEntry::zstd("Data\\secret.bin", DATA).encrypted())
        .entry(TestEntry::stored("Data\\unknown.bin", BLOCKS).with_encryption_field(2))
        .write_to(&archive)?;
    Ok(archive)
}

fn p4kx() -> Result<Command, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?)
}

fn json_of(output: &[u8]) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(output)?)
}

#[test]
fn verify_of_a_sound_archive_exits_with_0() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    p4kx()?
        .arg("verify")
        .arg(&archive)
        .args(["--filter", "good"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains(
            "verified 1, mismatched 0, failed 0, cancelled 0",
        ));
    Ok(())
}

#[test]
fn verify_reports_every_mismatch_and_exits_with_1() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    p4kx()?
        .arg("verify")
        .arg(&archive)
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "verified 2, mismatched 2, failed 0, cancelled 0",
        ))
        .stderr(predicate::str::contains("mismatch Data\\bad.bin: crc32c"))
        .stderr(predicate::str::contains("12345678 stored"))
        .stderr(predicate::str::contains("mismatch Data\\unknown.bin"));
    Ok(())
}

#[test]
fn verify_json_lists_the_mismatches() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let output = p4kx()?
        .arg("--json")
        .arg("verify")
        .arg(&archive)
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let report = json_of(&output)?;
    assert_eq!(report["verified"], 2, "the report counts the good Entries");
    let mismatched = report["mismatched"].as_array().ok_or("no mismatched")?;
    assert_eq!(mismatched.len(), 2, "two Entries mismatch");
    let bad = mismatched.first().ok_or("no first mismatch")?;
    assert_eq!(bad["path"], "Data\\bad.bin", "the path");
    assert_eq!(bad["kind"], "crc32c", "the checksum kind");
    assert_eq!(bad["expected"], "12345678", "the stored value");
    assert!(bad["actual"].is_string(), "the computed value");
    assert_eq!(
        report["failed"],
        Value::Array(Vec::new()),
        "nothing unreadable"
    );
    Ok(())
}

#[test]
fn verify_with_a_filter_that_matches_nothing_exits_with_2() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    p4kx()?
        .arg("verify")
        .arg(&archive)
        .args(["--filter", "no-such-entry"])
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn extract_checks_nothing_by_default() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    p4kx()?
        .arg("extract")
        .arg(&archive)
        .args(["--filter", "bad"])
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);

    assert_eq!(std::fs::read(out.join("Data/bad.bin"))?, DATA, "written");
    Ok(())
}

#[test]
fn extract_verify_counts_mismatches_writes_no_file_for_them_and_exits_with_1() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    p4kx()?
        .arg("extract")
        .arg(&archive)
        .args(["--filter", "Data\\", "--verify"])
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "extracted 2, skipped 0, failed 0, checksum mismatches 2, cancelled 0",
        ))
        .stderr(predicate::str::contains("mismatch Data\\bad.bin"));

    assert!(!out.join("Data/bad.bin").exists(), "no file for a mismatch");
    assert_eq!(
        std::fs::read(out.join("Data/good.bin"))?,
        DATA,
        "good Entry"
    );
    Ok(())
}

#[test]
fn extract_verify_json_lists_the_mismatches() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");

    let output = p4kx()?
        .arg("--json")
        .arg("extract")
        .arg(&archive)
        .args(["--filter", "bad", "--verify"])
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let report = json_of(&output)?;
    assert_eq!(report["extracted"], 0, "nothing is written");
    assert_eq!(
        report["failed"],
        Value::Array(Vec::new()),
        "no other failure"
    );
    let mismatched = report["mismatched"].as_array().ok_or("no mismatched")?;
    assert_eq!(mismatched.len(), 1, "one mismatch");
    assert_eq!(
        mismatched.first().map(|bad| &bad["path"]),
        Some(&Value::from("Data\\bad.bin")),
        "its path"
    );
    Ok(())
}

#[test]
fn extract_without_verify_has_no_mismatched_field() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let output = p4kx()?
        .arg("--json")
        .arg("extract")
        .arg(&archive)
        .args(["--filter", "good"])
        .arg("--out")
        .arg(dir.path().join("out"))
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();

    assert!(
        json_of(&output)?.get("mismatched").is_none(),
        "the field exists only with --verify"
    );
    Ok(())
}

#[test]
fn socpak_verify_uses_the_plain_crc32_of_the_container() -> TestResult {
    let dir = tempfile::tempdir()?;
    let container = TestContainer::new()
        .entry(TestEntry::stored("room\\good.xml", "<Good/>"))
        .entry(TestEntry::deflated("room\\bad.xml", "<Bad/>").with_crc32(0xDEAD_BEEF));
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\room.socpak", container.build()))
        .write_to(&archive)?;

    p4kx()?
        .args(["socpak", "verify"])
        .arg(&archive)
        .arg("Data\\room.socpak")
        .assert()
        .code(1)
        .stdout(predicate::str::contains("verified 1, mismatched 1"))
        .stderr(predicate::str::contains("mismatch room\\bad.xml: crc32"))
        .stderr(predicate::str::contains("deadbeef stored"));

    let out = dir.path().join("out");
    p4kx()?
        .args(["socpak", "extract"])
        .arg(&archive)
        .arg("Data\\room.socpak")
        .arg("--verify")
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1);
    assert!(
        out.join("room/good.xml").exists(),
        "the good Entry is written"
    );
    assert!(!out.join("room/bad.xml").exists(), "the bad one is not");
    Ok(())
}

#[test]
fn verify_reports_a_wrong_sha_256_with_64_hex_digits() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\good.bin", DATA).with_sha256())
        .entry(TestEntry::zstd("Data\\bad.bin", DATA).with_sha256_value([0xAB; 32]))
        .write_to(&archive)?;

    let output = p4kx()?
        .arg("--json")
        .arg("verify")
        .arg(&archive)
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let report = json_of(&output)?;
    assert_eq!(
        report["verified"], 1,
        "the Entry with the right digest passes"
    );
    let bad = report["mismatched"].get(0).ok_or("no mismatch")?;
    assert_eq!(bad["path"], "Data\\bad.bin", "the path");
    assert_eq!(bad["kind"], "sha256", "the checksum kind");
    assert_eq!(bad["expected"], "ab".repeat(32), "the stated digest");
    assert_eq!(
        bad["actual"].as_str().map(str::len),
        Some(64),
        "the computed digest"
    );
    Ok(())
}
