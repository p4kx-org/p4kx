// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_archive::{Archive, ArchiveError, KeyDemandSnapshot};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, other_test_key, test_key};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

fn secret(path: &str) -> TestEntry {
    TestEntry::deflated(path, b"synthetic encrypted Entry").encrypted()
}

fn open(builder: &ArchiveBuilder) -> Result<(TempDir, Archive), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    builder.write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

fn refused(archive: &Archive, index: usize) -> Result<ArchiveError, Box<dyn Error>> {
    let entry = archive.entries().get(index).ok_or("physical Entry")?;
    archive
        .open_entry(entry)
        .err()
        .ok_or_else(|| "encrypted Entry unexpectedly opened".into())
}

const fn counts(snapshot: &KeyDemandSnapshot) -> [usize; 3] {
    [snapshot.missing, snapshot.wrong, snapshot.unread()]
}

fn nested() -> ArchiveBuilder {
    let leaf = ArchiveBuilder::new().entry(secret("same.xml")).build();
    let middle = TestContainer::new()
        .entry(TestEntry::stored("same.socpak", leaf))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("same.socpak", middle.clone()))
        .entry(TestEntry::stored("same.socpak", middle))
}

fn innermost(archive: &Archive, index: usize) -> Result<Archive, Box<dyn Error>> {
    let outer = archive.open_container(archive.entries().get(index).ok_or("outer Entry")?)?;
    Ok(outer.open_container(outer.entries().first().ok_or("inner Entry")?)?)
}

#[test]
fn duplicate_root_paths_count_as_two_physical_entries() -> TestResult {
    let (_folder, archive) = open(
        &ArchiveBuilder::new()
            .entry(secret("same.xml"))
            .entry(secret("same.xml")),
    )?;
    for index in [0, 1] {
        assert!(refused(&archive, index)?.is_key_required());
    }
    let snapshot = archive.key_demand().snapshot();
    assert_eq!(snapshot.first_missing.as_deref(), Some("same.xml"));
    assert_eq!(counts(&snapshot), [2, 0, 2], "separate directory positions");
    Ok(())
}

#[test]
fn identical_inner_paths_and_display_locations_keep_the_full_physical_chain() -> TestResult {
    let (_folder, archive) = open(&nested())?;
    let first = innermost(&archive, 0)?;
    let second = innermost(&archive, 1)?;
    assert_eq!(
        first.location(),
        second.location(),
        "the fixture deliberately has identical display paths"
    );
    for child in [&first, &second] {
        assert!(refused(child, 0)?.is_key_required());
    }
    assert_eq!(counts(&archive.key_demand().snapshot()), [2, 0, 2]);
    Ok(())
}

#[test]
fn reopening_the_same_container_reuses_its_physical_entry_identity() -> TestResult {
    let (_folder, archive) = open(&nested())?;
    let scope = archive.key_demand().scope();
    for _ in 0..3 {
        let child = innermost(&archive, 0)?;
        assert!(refused(&child, 0)?.is_key_required());
    }
    assert_eq!(counts(&archive.key_demand().snapshot()), [1, 0, 1]);
    assert_eq!(counts(&scope.snapshot()), [1, 0, 1]);
    Ok(())
}

#[test]
fn repeated_reads_and_ranged_reads_count_the_same_physical_entry_once() -> TestResult {
    let (_folder, archive) = open(&ArchiveBuilder::new().entry(secret("same.xml")))?;
    for _ in 0..3 {
        assert!(refused(&archive, 0)?.is_key_required());
    }
    let entry = archive.entries().first().ok_or("physical Entry")?;
    let error = archive.open_entry_at(entry, 1).err().ok_or("ranged read")?;
    assert!(error.is_key_required());
    assert_eq!(counts(&archive.key_demand().snapshot()), [1, 0, 1]);
    Ok(())
}

#[test]
fn missing_then_wrong_preserves_both_causes_but_one_distinct_unread_entry() -> TestResult {
    let (_folder, archive) = open(&ArchiveBuilder::new().entry(secret("same.xml")))?;
    let scope = archive.key_demand().scope();
    assert!(refused(&archive, 0)?.is_key_required());
    archive.set_key(other_test_key());
    assert!(refused(&archive, 0)?.is_wrong_key());
    let snapshot = archive.key_demand().snapshot();
    assert_eq!(snapshot.first_missing.as_deref(), Some("same.xml"));
    assert_eq!(snapshot.first_wrong.as_deref(), Some("same.xml"));
    assert_eq!(counts(&snapshot), [1, 1, 1]);
    assert_eq!(counts(&scope.snapshot()), [1, 1, 1]);
    archive.set_key(test_key());
    let entry = archive.entries().first().ok_or("physical Entry")?;
    drop(archive.open_entry(entry)?);
    assert_eq!(counts(&archive.key_demand().snapshot()), [1, 1, 1]);
    Ok(())
}

#[test]
fn partly_overlapping_causes_and_later_scopes_have_independent_union_counts() -> TestResult {
    let (_folder, archive) = open(
        &ArchiveBuilder::new()
            .entry(secret("first.xml"))
            .entry(secret("overlap.xml"))
            .entry(secret("last.xml")),
    )?;
    let all = archive.key_demand().scope();
    for index in [0, 1] {
        assert!(refused(&archive, index)?.is_key_required());
    }
    let later = archive.key_demand().scope();
    archive.set_key(other_test_key());
    for index in [1, 2] {
        assert!(refused(&archive, index)?.is_wrong_key());
    }
    assert_eq!(counts(&archive.key_demand().snapshot()), [2, 2, 3]);
    assert_eq!(counts(&all.snapshot()), [2, 2, 3]);
    assert_eq!(counts(&later.snapshot()), [0, 2, 2]);
    Ok(())
}

#[test]
fn root_and_child_entries_with_the_same_index_and_path_are_distinct() -> TestResult {
    let leaf = ArchiveBuilder::new().entry(secret("same.xml")).build();
    let (_folder, archive) = open(
        &ArchiveBuilder::new()
            .entry(secret("same.xml"))
            .entry(TestEntry::stored("leaf.socpak", leaf)),
    )?;
    assert!(refused(&archive, 0)?.is_key_required());
    let child = archive.open_container(archive.entries().get(1).ok_or("container Entry")?)?;
    assert!(refused(&child, 0)?.is_key_required());
    assert_eq!(counts(&archive.key_demand().snapshot()), [2, 0, 2]);
    Ok(())
}
