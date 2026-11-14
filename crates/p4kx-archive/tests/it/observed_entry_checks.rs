// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Borrowed proof assertions and original binding hash contexts.

use std::collections::HashSet;
use std::ptr;

use p4kx_archive::{Archive, Entry, ObservedEntry, ObservedEntryBinding, SourceEncryption};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::observed_entry as support;
use crate::observed_entry::TestResult;

pub(super) fn borrowed_facts(view: &ObservedEntry<'_>, actual: &Entry) {
    assert!(
        ptr::eq(view.entry(), actual),
        "observation returns the actual admitted Entry borrow"
    );
    assert!(
        ptr::eq(view.entry().metadata(), actual.metadata()),
        "stored metadata is borrowed rather than cloned"
    );
    assert_eq!(
        view.entry_index(),
        0,
        "the original fixed-width ordinal is retained"
    );
    assert_eq!(
        view.ancestors().len(),
        0,
        "root Entry has no invented containing Entries"
    );
    assert_eq!(
        view.stored_encryption(),
        SourceEncryption::Encrypted,
        "stored evidence does not require a payload read"
    );
}

pub(super) fn unproved_views(
    one_binding: &ObservedEntryBinding,
    unproved_one: &Archive,
    unproved_two: &Archive,
) -> TestResult {
    let first_view = unproved_one
        .observed_entry(support::first(unproved_one)?)
        .ok_or("first unproved view")?;
    let first_binding = first_view.binding();
    assert!(
        first_binding.has_unknown_ancestry(),
        "the detached containing step remains unproved"
    );
    assert!(
        first_binding.is_same_entry(&first_view.binding()),
        "one actual view has stable opaque identity"
    );
    assert!(
        !first_binding.is_same_entry(
            &unproved_two
                .observed_entry(support::first(unproved_two)?)
                .ok_or("second unproved view")?
                .binding()
        ),
        "equal missing ancestry cannot alias independently opened views"
    );
    assert!(
        !first_binding.is_same_entry(one_binding),
        "unproved ancestry cannot acquire proven-view identity"
    );
    assert!(
        first_binding.ancestor(0).is_none(),
        "an unproved container does not produce fabricated original Entry proof"
    );
    Ok(())
}

fn known_hash_contexts(
    bindings: &mut HashSet<ObservedEntryBinding>,
    root: &Archive,
    container: &Entry,
) -> TestResult {
    let one = root.open_container(container)?;
    let two = root.open_container(container)?;
    let original = one
        .observed_entry(support::first(&one)?)
        .ok_or("first original proven view")?
        .binding();
    let reopened = two
        .observed_entry(support::first(&two)?)
        .ok_or("reopened original proven view")?
        .binding();
    assert_eq!(
        original, reopened,
        "Eq has the same original-owner/coordinate contract as is_same_entry"
    );

    assert!(
        bindings.insert(original),
        "a new genuine context is admitted once"
    );
    assert!(
        bindings.contains(&reopened),
        "genuine reopens remain findable in the hash index"
    );
    assert!(
        !bindings.insert(reopened),
        "equal proven contexts do not occupy two index entries"
    );
    Ok(())
}

fn unproved_hash_contexts(
    bindings: &mut HashSet<ObservedEntryBinding>,
    root: &Archive,
    container: &Entry,
) -> TestResult {
    let detached = container.clone();
    let middle = root.open_container(&detached)?;
    let middle_binding = middle
        .observed_entry(support::first(&middle)?)
        .ok_or("original unproved view")?
        .binding();
    let deeper = middle.open_container(support::first(&middle)?)?;
    let inherited = deeper
        .observed_entry(support::first(&deeper)?)
        .ok_or("actual deeper view")?
        .binding()
        .ancestor(1)
        .ok_or("actual original prefix")?;
    assert_eq!(
        middle_binding, inherited,
        "a deeper ancestry allocation keeps the original shorter context"
    );
    assert!(
        bindings.insert(middle_binding.clone()),
        "the original unproved token is a separate context"
    );
    assert!(
        bindings.contains(&inherited),
        "a shared shorter original prefix remains findable by Hash and Eq"
    );
    assert!(
        !bindings.insert(inherited),
        "the same original prefix has one index entry"
    );

    let different = root.open_container(&detached)?;
    let different_binding = different
        .observed_entry(support::first(&different)?)
        .ok_or("different unproved view")?
        .binding();
    assert_ne!(
        middle_binding, different_binding,
        "independently created unproved tokens remain distinct"
    );
    assert!(
        bindings.insert(different_binding),
        "equal path/ordinals cannot merge different unproved contexts"
    );
    Ok(())
}

fn retained_hash_owner(
    bindings: &mut HashSet<ObservedEntryBinding>,
    root_binding: &ObservedEntryBinding,
    bytes: &[u8],
) -> TestResult {
    let retained = {
        let (_foreign_directory, foreign) = support::open(bytes)?;
        foreign
            .observed_entry(support::first(&foreign)?)
            .ok_or("different original root")?
            .binding()
    };
    assert_ne!(
        root_binding, &retained,
        "equal bytes and coordinates do not merge distinct original root owners"
    );
    assert!(
        !bindings.contains(&retained),
        "a foreign owner is absent even after its Archive is dropped"
    );
    assert!(
        bindings.insert(retained.clone()),
        "the owned binding keeps its original owner alive"
    );
    assert!(
        bindings.contains(&retained),
        "owned identity remains hashable after the original Archive lifetime"
    );
    Ok(())
}

pub(super) fn original_entry_bindings_are_hash_keys_across_reopens_prefixes_and_retained_owners()
-> TestResult {
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"same"))
        .build();
    let middle_bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("inner.pak", leaf))
        .build();
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("outer.pak", middle_bytes))
        .build();
    let (_directory, root) = support::open(&bytes)?;
    let container = support::first(&root)?;
    let mut bindings = HashSet::new();
    known_hash_contexts(&mut bindings, &root, container)?;
    let root_binding = root
        .observed_entry(container)
        .ok_or("actual root Entry")?
        .binding();
    assert!(
        bindings.insert(root_binding.clone()),
        "the root has its own original context"
    );
    unproved_hash_contexts(&mut bindings, &root, container)?;
    retained_hash_owner(&mut bindings, &root_binding, &bytes)?;
    assert_eq!(
        bindings.len(),
        5,
        "only genuinely distinct original Entry contexts occupy the index"
    );
    Ok(())
}
