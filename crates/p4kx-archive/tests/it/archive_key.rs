// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Archive key: reading without one, with the right one and with a wrong one.

use std::error::Error;
use std::io::Read;
use std::path::Path;

use p4kx_archive::{Archive, ArchiveError, Entry, KeyCheck};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, other_test_key, test_key};

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

pub(crate) const SECRET: &[u8] = b"<secret data of the Archive/>";
pub(crate) const SOCPAK: &str = "Data\\room.socpak";

pub(crate) fn builder() -> ArchiveBuilder {
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\plain.xml", b"<plain/>".to_vec()))
        .entry(TestEntry::deflated("Data\\secret.xml", SECRET.to_vec()).encrypted())
}

pub(crate) fn open(dir: &Path, builder: &ArchiveBuilder) -> Result<Archive, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    builder.write_to(&path)?;
    Ok(Archive::open(&path)?)
}

pub(crate) fn read(archive: &Archive, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let entry = find(archive, path)?;
    let mut data = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut data)?;
    Ok(data)
}

pub(crate) fn find<'a>(archive: &'a Archive, path: &str) -> Result<&'a Entry, Box<dyn Error>> {
    Ok(archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or_else(|| format!("no Entry {path}"))?)
}

#[test]
fn without_a_key_everything_unencrypted_still_works() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(dir.path(), &builder())?;

    assert_eq!(archive.entries().len(), 2, "listing");
    assert!(!archive.has_key(), "no key set");
    assert_eq!(
        read(&archive, "Data\\plain.xml")?,
        b"<plain/>",
        "unencrypted Entry"
    );
    Ok(())
}

#[test]
fn an_encrypted_entry_without_a_key_is_a_typed_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(dir.path(), &builder())?;
    let entry = find(&archive, "Data\\secret.xml")?;

    let error = archive.open_entry(entry).err().ok_or("opened")?;
    let ranged = archive.open_entry_at(entry, 4).err().ok_or("opened")?;

    assert!(
        matches!(&error, ArchiveError::KeyRequired { entry, .. } if entry == "Data\\secret.xml"),
        "key required: {error}"
    );
    assert!(error.is_key_required(), "classified");
    assert!(ranged.is_key_required(), "also from an offset");
    Ok(())
}

#[test]
fn the_right_key_reads_the_entry() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        {
            let path = dir.path().join("Data.p4k");
            builder().write_to(&path)?;
            path
        },
        Some(test_key()),
    )?;

    assert!(archive.has_key(), "key set");
    assert_eq!(read(&archive, "Data\\secret.xml")?, SECRET, "decrypted");
    Ok(())
}

#[test]
fn a_key_set_later_applies_to_an_open_archive() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(dir.path(), &builder())?;
    assert!(read(&archive, "Data\\secret.xml").is_err(), "before");

    archive.set_key(test_key());
    assert_eq!(read(&archive, "Data\\secret.xml")?, SECRET, "after");

    archive.clear_key();
    assert!(!archive.has_key(), "cleared");
    assert!(read(&archive, "Data\\secret.xml").is_err(), "after removal");
    Ok(())
}

#[test]
fn a_wrong_key_is_detected_instead_of_returning_garbage() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(dir.path(), &builder())?;
    archive.set_key(other_test_key());

    let error = archive
        .open_entry(find(&archive, "Data\\secret.xml")?)
        .err()
        .ok_or("opened")?;

    assert!(
        matches!(&error, ArchiveError::WrongKey { .. }),
        "wrong key: {error}"
    );
    assert!(error.is_wrong_key(), "classified");
    assert!(
        !format!("{error} {error:?}").contains("p4kx-other"),
        "the key is not in the error"
    );
    Ok(())
}

#[test]
fn a_wrong_key_is_detected_on_stored_data_by_its_checksum() -> TestResult {
    let dir = tempfile::tempdir()?;
    let builder = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\secret.bin", SECRET.to_vec()).encrypted());
    let archive = open(dir.path(), &builder)?;

    assert!(
        archive
            .verify_key(&other_test_key())
            .err()
            .ok_or("accepted")?
            .is_wrong_key(),
        "wrong key"
    );
    assert_eq!(
        archive.verify_key(&test_key())?,
        KeyCheck::Verified,
        "right key"
    );
    Ok(())
}

#[test]
fn a_corrupt_smallest_entry_does_not_turn_the_right_key_into_a_wrong_one() -> TestResult {
    let dir = tempfile::tempdir()?;
    let builder = ArchiveBuilder::new()
        .entry(
            TestEntry::stored("Data\\tiny.bin", b"tiny".to_vec())
                .encrypted()
                .with_crc32c(1),
        )
        .entry(TestEntry::stored("Data\\secret.bin", SECRET.to_vec()).encrypted());
    let archive = open(dir.path(), &builder)?;

    assert_eq!(
        archive.verify_key(&test_key())?,
        KeyCheck::Verified,
        "the next candidate decides"
    );
    archive.set_key(test_key());
    assert_eq!(read(&archive, "Data\\secret.bin")?, SECRET, "readable");
    assert!(
        archive
            .verify_key(&other_test_key())
            .err()
            .ok_or("accepted")?
            .is_wrong_key(),
        "a wrong key still fails on every candidate"
    );
    Ok(())
}

#[test]
fn verifying_a_key_sets_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(dir.path(), &builder())?;

    archive.verify_key(&test_key())?;

    assert!(!archive.has_key(), "still no key");
    Ok(())
}

#[test]
fn an_archive_without_encrypted_entries_cannot_check_a_key() -> TestResult {
    let dir = tempfile::tempdir()?;
    let builder = ArchiveBuilder::new().entry(TestEntry::stored("a", b"a".to_vec()));
    let archive = open(dir.path(), &builder)?;

    assert_eq!(
        archive.verify_key(&other_test_key())?,
        KeyCheck::Unchecked,
        "nothing to refute it"
    );
    Ok(())
}

#[test]
fn a_missing_or_wrong_key_is_requested_once_for_a_window_to_ask_for_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(dir.path(), &builder())?;
    assert!(!archive.take_key_request(), "nothing was needed yet");
    assert!(read(&archive, "Data\\plain.xml").is_ok(), "unencrypted");
    assert!(
        !archive.take_key_request(),
        "an unencrypted Entry needs none"
    );

    assert!(read(&archive, "Data\\secret.xml").is_err(), "no key");
    assert!(archive.take_key_request(), "requested");
    assert!(!archive.take_key_request(), "only once");

    archive.set_key(other_test_key());
    assert!(read(&archive, "Data\\secret.xml").is_err(), "wrong key");
    assert!(
        archive.take_key_request(),
        "requested again for the wrong key"
    );
    assert!(read(&archive, "Data\\secret.xml").is_err(), "still wrong");
    assert!(!archive.take_key_request(), "the same wrong key asks once");

    archive.set_key(test_key());
    assert!(read(&archive, "Data\\secret.xml").is_ok(), "right key");
    assert!(!archive.take_key_request(), "nothing to ask");
    Ok(())
}

#[test]
fn containers_share_the_key_of_their_archive() -> TestResult {
    let dir = tempfile::tempdir()?;
    let room = TestContainer::new().entry(TestEntry::stored("room.xml", b"<Room/>".to_vec()));
    let builder = builder().entry(TestEntry::deflated(SOCPAK, room.build()).encrypted());
    let archive = open(dir.path(), &builder)?;
    let socpak = find(&archive, SOCPAK)?;

    let error = archive.open_container(socpak).err().ok_or("opened")?;
    assert!(
        error.is_key_required(),
        "key required through the container error: {error}"
    );

    archive.set_key(test_key());
    let container = archive.open_container(socpak)?;
    archive.clear_key();
    container.set_key(test_key());
    assert!(archive.has_key(), "the container shares the key slot");
    assert_eq!(container.entries().len(), 1, "container listing");
    Ok(())
}
