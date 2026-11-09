// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Borrowed source proof, exact original ownership and containing-view identity.

use std::error::Error;
use std::fs;

use p4kx_archive::{Archive, Entry};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn open(bytes: &[u8]) -> TestResult<(TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture.p4k");
    fs::write(&path, bytes)?;
    Ok((directory, Archive::open(path)?))
}

fn first(archive: &Archive) -> TestResult<&Entry> {
    archive
        .entries()
        .first()
        .ok_or_else(|| "actual Entry missing".into())
}

#[test]
fn observed_entry_borrows_actual_facts_and_rejects_equal_foreign_or_detached_entries() -> TestResult
{
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("same.bin", b"same").encrypted())
        .build();
    let (_directory, archive) = open(&bytes)?;
    let (_other_directory, other) = open(&bytes)?;
    let actual = first(&archive)?;
    let detached = actual.clone();
    assert!(
        archive.observed_entry(&detached).is_none(),
        "detached equal metadata grants no original membership"
    );
    assert!(
        archive.observed_entry(first(&other)?).is_none(),
        "same bytes and index in another owner grant no membership"
    );
    let view = archive
        .observed_entry(actual)
        .ok_or("actual borrowed proof")?;
    checks::borrowed_facts(&view, actual);
    let binding = view.binding();
    let snapshot = view.snapshot();
    assert!(
        binding.is_same_entry(&snapshot.binding()),
        "optional full snapshot retains the same original binding"
    );
    assert!(
        !binding.is_same_entry(
            &other
                .observed_entry(first(&other)?)
                .ok_or("other original proof")?
                .binding()
        ),
        "same-byte roots stay distinct"
    );
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "observing source proof does not access encrypted bytes"
    );
    drop(archive);
    assert!(
        binding.is_same_entry(&snapshot.binding()),
        "owned proof remains valid after the Archive borrow ends"
    );
    Ok(())
}

#[test]
fn known_reopened_containers_match_but_distinct_unproved_views_do_not() -> TestResult {
    let inner = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"leaf"))
        .build();
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("room.pak", inner))
        .build();
    let (_directory, archive) = open(&bytes)?;
    let container = first(&archive)?;
    let one = archive.open_container(container)?;
    let two = archive.open_container(container)?;
    let one_binding = one
        .observed_entry(first(&one)?)
        .ok_or("first original view")?
        .binding();
    let two_binding = two
        .observed_entry(first(&two)?)
        .ok_or("reopened original view")?
        .binding();
    assert!(
        one_binding.is_same_entry(&two_binding),
        "same original root and proven container indices identify one physical occurrence"
    );
    let detached = container.clone();
    let unproved_one = archive.open_container(&detached)?;
    let unproved_two = archive.open_container(&detached)?;
    checks::unproved_views(&one_binding, &unproved_one, &unproved_two)?;
    assert!(
        one_binding.ancestor(1).is_none(),
        "ancestor position excludes the leaf"
    );
    assert!(
        one_binding.ancestor(usize::MAX).is_none(),
        "ancestor bounds are checked before indexing"
    );
    let actual_container = archive
        .observed_entry(container)
        .ok_or("actual root container")?
        .binding();
    assert!(
        one_binding
            .ancestor(0)
            .ok_or("original containing proof")?
            .is_same_entry(&actual_container),
        "ancestor proof retains the exact original root occurrence"
    );
    Ok(())
}

#[test]
fn unknown_original_prefix_keeps_its_owner_when_shared_with_a_deeper_container() -> TestResult {
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"same"))
        .build();
    let middle = ArchiveBuilder::new()
        .entry(TestEntry::stored("inner.pak", leaf))
        .build();
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("outer.pak", middle))
        .build();
    let (_directory, root) = open(&bytes)?;
    let detached = first(&root)?.clone();
    let middle_view = root.open_container(&detached)?;
    let middle_binding = middle_view
        .observed_entry(first(&middle_view)?)
        .ok_or("actual middle view")?
        .binding();
    let deeper = middle_view.open_container(first(&middle_view)?)?;
    let deeper_binding = deeper
        .observed_entry(first(&deeper)?)
        .ok_or("actual deeper view")?
        .binding();
    assert!(
        deeper_binding
            .ancestor(1)
            .ok_or("original middle ancestor")?
            .is_same_entry(&middle_binding),
        "sharing the original unproved prefix preserves its actual owner across a new ancestry slice"
    );
    assert!(
        deeper_binding.ancestor(0).is_none(),
        "an unproved containing step stays unproved"
    );
    let different = root.open_container(&detached)?;
    let different_binding = different
        .observed_entry(first(&different)?)
        .ok_or("different middle view")?
        .binding();
    assert!(
        !deeper_binding
            .ancestor(1)
            .ok_or("original middle ancestor")?
            .is_same_entry(&different_binding),
        "equal unknown prefixes from independent opens never acquire the original opaque owner"
    );
    Ok(())
}

#[test]
fn original_entry_bindings_are_hash_keys_across_reopens_prefixes_and_retained_owners() -> TestResult
{
    checks::original_entry_bindings_are_hash_keys_across_reopens_prefixes_and_retained_owners()
}

#[path = "observed_entry_checks.rs"]
mod checks;
