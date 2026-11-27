// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The key demand and the key generation of an Archive.

use p4kx_archive::KeyDemandSnapshot;
use p4kx_testkit::{TestContainer, TestEntry, other_test_key, test_key};

use crate::archive_key::{SOCPAK, TestResult, builder, find, open, read};

#[test]
fn the_key_generation_changes_with_the_key_and_not_with_the_same_key_again() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(dir.path(), &builder())?;
    let none = archive.key_generation();
    archive.clear_key();
    assert_eq!(
        archive.key_generation(),
        none,
        "clearing no key changes none"
    );

    archive.set_key(other_test_key());
    let wrong = archive.key_generation();
    assert_ne!(wrong, none, "a key was set");
    archive.set_key(other_test_key());
    assert_eq!(archive.key_generation(), wrong, "the same key again");

    archive.set_key(test_key());
    let right = archive.key_generation();
    assert_ne!(right, wrong, "a different key");
    archive.clear_key();
    assert_ne!(archive.key_generation(), right, "the key was cleared");
    Ok(())
}

#[test]
fn containers_share_the_key_generation_of_their_archive() -> TestResult {
    let dir = tempfile::tempdir()?;
    let room = TestContainer::new().entry(TestEntry::stored("room.xml", b"<Room/>".to_vec()));
    let builder = builder().entry(TestEntry::deflated(SOCPAK, room.build()).encrypted());
    let archive = open(dir.path(), &builder)?;
    archive.set_key(test_key());
    let container = archive.open_container(find(&archive, SOCPAK)?)?;
    let before = archive.key_generation();

    container.set_key(other_test_key());

    assert_ne!(archive.key_generation(), before, "one slot for both");
    Ok(())
}

#[test]
fn the_key_demand_counts_distinct_entries_and_resets_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let builder =
        builder().entry(TestEntry::stored("Data\\other.xml", b"<o/>".to_vec()).encrypted());
    let archive = open(dir.path(), &builder)?;
    let demand = archive.key_demand();
    assert_eq!(
        demand.snapshot(),
        KeyDemandSnapshot::default(),
        "nothing yet"
    );

    assert!(read(&archive, "Data\\plain.xml").is_ok(), "unencrypted");
    assert!(read(&archive, "Data\\secret.xml").is_err(), "no key");
    assert!(read(&archive, "Data\\secret.xml").is_err(), "again");
    assert!(read(&archive, "Data\\other.xml").is_err(), "a second Entry");
    assert!(archive.take_key_request(), "the one-shot flag is separate");

    let seen = demand.snapshot();
    assert_eq!(seen.missing, 2, "distinct Entries");
    assert_eq!(seen.wrong, 0, "no wrong key");
    assert_eq!(seen.unread(), 2, "total");
    assert_eq!(
        seen.first_missing.as_deref(),
        Some("Data\\secret.xml"),
        "first asked"
    );
    assert_eq!(
        archive.key_demand().snapshot(),
        seen,
        "every handle shares it"
    );

    archive.set_key(test_key());
    assert!(read(&archive, "Data\\secret.xml").is_ok(), "right key");
    assert_eq!(demand.snapshot(), seen, "setting a key resets nothing");
    Ok(())
}

#[test]
fn the_key_demand_counts_entries_a_wrong_key_did_not_open() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(dir.path(), &builder())?;
    archive.set_key(other_test_key());

    assert!(read(&archive, "Data\\secret.xml").is_err(), "wrong key");
    assert!(
        read(&archive, "Data\\secret.xml").is_err(),
        "wrong key again"
    );

    let seen = archive.key_demand().snapshot();
    assert_eq!((seen.missing, seen.wrong), (0, 1), "one Entry, wrong key");
    assert_eq!(
        seen.first_wrong.as_deref(),
        Some("Data\\secret.xml"),
        "its path"
    );
    Ok(())
}

#[test]
fn the_key_demand_is_shared_with_containers() -> TestResult {
    let dir = tempfile::tempdir()?;
    let room = TestContainer::new().entry(TestEntry::stored("room.xml", b"<Room/>".to_vec()));
    let builder = builder().entry(TestEntry::deflated(SOCPAK, room.build()).encrypted());
    let archive = open(dir.path(), &builder)?;

    assert!(
        archive.open_container(find(&archive, SOCPAK)?).is_err(),
        "no key"
    );

    assert_eq!(
        archive.key_demand().snapshot().missing,
        1,
        "counted on the Archive"
    );
    Ok(())
}

#[test]
fn a_demand_scope_counts_from_its_opening_until_it_is_dropped() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(dir.path(), &builder())?;
    let demand = archive.key_demand();
    assert!(read(&archive, "Data\\secret.xml").is_err(), "before");

    let scope = demand.scope();
    assert_eq!(
        scope.snapshot(),
        KeyDemandSnapshot::default(),
        "starts empty"
    );
    assert!(read(&archive, "Data\\secret.xml").is_err(), "asked again");
    assert_eq!(
        scope.snapshot().missing,
        1,
        "an Entry asked for before the scope counts again in it"
    );
    assert_eq!(demand.snapshot().missing, 1, "the total stays distinct");

    let later = demand.scope();
    drop(scope);
    assert_eq!(later.snapshot(), KeyDemandSnapshot::default(), "its own");
    Ok(())
}
