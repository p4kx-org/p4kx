// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The ship base states whether the Archive key was there for the encrypted vehicle files.

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{ArchiveKeyState, BuiltShipBase, ShipBaseOptions, build_ship_base};
use p4kx_extraction::Archive;
use p4kx_testkit::{other_test_key, test_key};

use crate::ship_base_fixture::write_encrypted_vehicle_archive;

type TestResult = Result<(), Box<dyn Error>>;

fn build(key: Option<p4kx_extraction::ArchiveKey>) -> Result<BuiltShipBase, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(write_encrypted_vehicle_archive(dir.path())?, key)?;
    let options = ShipBaseOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    Ok(build_ship_base(&archive, &options)?)
}

fn source_of(built: &BuiltShipBase) -> Result<serde_json::Value, Box<dyn Error>> {
    let mut out = Vec::new();
    built.write_json(&mut out)?;
    Ok(serde_json::from_slice(&out)?)
}

#[test]
fn without_a_key_the_ship_base_says_it_was_built_without_one() -> TestResult {
    let built = build(None)?;

    let key = built.archive_key();
    assert_eq!(key.state(), ArchiveKeyState::Missing, "state");
    assert_eq!(key.entries_unread(), 1, "the vehicle file of ACME_Alpha");
    let source = source_of(&built)?;
    assert_eq!(source["source"]["archiveKey"], "missing", "file");
    assert_eq!(source["source"]["encryptedEntriesUnread"], 1, "file count");
    Ok(())
}

#[test]
fn with_the_key_nothing_is_unread() -> TestResult {
    let built = build(Some(test_key()))?;

    assert_eq!(built.archive_key().state(), ArchiveKeyState::Given, "state");
    assert_eq!(built.archive_key().entries_unread(), 0, "count");
    let source = source_of(&built)?;
    assert_eq!(source["source"]["archiveKey"], "given", "file");
    assert_eq!(source["source"]["encryptedEntriesUnread"], 0, "file count");
    Ok(())
}

#[test]
fn a_key_that_does_not_fit_is_wrong_not_missing() -> TestResult {
    let built = build(Some(other_test_key()))?;

    assert_eq!(built.archive_key().state(), ArchiveKeyState::Wrong, "state");
    assert_eq!(built.archive_key().entries_unread(), 1, "count");
    Ok(())
}
