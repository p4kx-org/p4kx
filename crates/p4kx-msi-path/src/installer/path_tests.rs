// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::claim::Claim;
use crate::installer::fault::Fault;
use crate::installer::lease::Lease;
use crate::installer::path_edit;
use crate::installer::raw_path::{RawPath, StringKind};
use crate::installer::state::Request;

use super::{ENTRY, PRODUCT, TestResult};
use crate::installer::tests;

#[test]
fn every_original_existence_kind_and_empty_segment_round_trips() -> TestResult {
    for original in [
        RawPath::Absent,
        tests::path("", StringKind::Plain),
        tests::path("", StringKind::Expandable),
        tests::path("A;B", StringKind::Plain),
        tests::path("A;B", StringKind::Expandable),
        tests::path(";A;B", StringKind::Expandable),
        tests::path("A;B;", StringKind::Expandable),
        tests::path(";;A;;B;;", StringKind::Expandable),
    ] {
        let mut store = tests::memory(original.clone());
        let id = tests::begin(
            &mut store,
            Request::Install {
                product: PRODUCT,
                lease: tests::lease()?,
            },
        )?;
        assert_eq!(
            store.state.path, original,
            "write-ahead preparation does not edit PATH"
        );
        tests::apply(&mut store, id)?;
        tests::commit(&mut store, id)?;
        tests::remove(&mut store, PRODUCT)?;
        assert_eq!(
            store.state.path, original,
            "original exists/type/raw bytes survive the full lifecycle"
        );
    }
    Ok(())
}

#[test]
fn source_utf16_and_terminator_bits_are_not_text_normalised() -> TestResult {
    let original = RawPath::Present {
        kind: StringKind::Expandable,
        bytes: vec![0, 0xd8, b';', 0, 0, 0, 0, 0, 7, 0],
    };
    let mut store = tests::memory(original.clone());
    tests::install(&mut store)?;
    tests::remove(&mut store, PRODUCT)?;
    assert_eq!(
        store.state.path, original,
        "unpaired UTF16 and extra source terminator bytes remain exact"
    );
    Ok(())
}

#[test]
fn all_foreign_bytes_outside_the_single_owned_splice_survive() -> TestResult {
    let mut store = tests::memory(tests::path(";A;;B;", StringKind::Expandable));
    tests::install(&mut store)?;
    store.state.path = tests::path(&format!("F;;{ENTRY};;A;;B;;G;"), StringKind::Plain);
    tests::remove(&mut store, PRODUCT)?;
    assert_eq!(
        store.state.path,
        tests::path("F;;;A;;B;;G;", StringKind::Plain),
        "only the owned element and one joining delimiter are removed"
    );
    Ok(())
}

#[test]
fn external_deletion_does_not_restore_the_obsolete_snapshot() -> TestResult {
    let mut store = tests::memory(tests::path("A", StringKind::Expandable));
    tests::install(&mut store)?;
    store.state.path = RawPath::Absent;
    tests::remove(&mut store, PRODUCT)?;
    assert_eq!(
        store.state.path,
        RawPath::Absent,
        "the user's deletion is retained"
    );
    Ok(())
}

#[test]
fn foreign_kind_change_is_preserved_even_when_the_source_was_absent() -> TestResult {
    let mut store = tests::memory(RawPath::Absent);
    tests::install(&mut store)?;
    store.state.path = tests::path(ENTRY, StringKind::Expandable);
    tests::remove(&mut store, PRODUCT)?;
    assert_eq!(
        store.state.path,
        tests::path("", StringKind::Expandable),
        "a new foreign type declaration is not erased"
    );
    Ok(())
}

#[test]
fn an_owned_element_moved_to_the_end_removes_only_one_separator() -> TestResult {
    let mut store = tests::memory(tests::path("A", StringKind::Plain));
    tests::install(&mut store)?;
    store.state.path = tests::path(&format!(";F;;A;;{ENTRY}"), StringKind::Plain);
    tests::remove(&mut store, PRODUCT)?;
    assert_eq!(
        store.state.path,
        tests::path(";F;;A;", StringKind::Plain),
        "foreign empty elements stay represented"
    );
    Ok(())
}

#[test]
fn a_preexisting_entry_is_borrowed_without_rewriting_or_removing_it() -> TestResult {
    let original = tests::path(ENTRY, StringKind::Plain);
    let mut store = tests::memory(original.clone());
    tests::install(&mut store)?;
    assert!(
        matches!(store.state.owner.as_ref(), Some(owner) if owner.claim == Claim::Borrowed),
        "using a foreign existing entry never creates an inserted claim"
    );
    assert_eq!(
        store.state.path, original,
        "a borrowed entry changes no source bytes"
    );
    tests::remove(&mut store, PRODUCT)?;
    assert_eq!(
        store.state.path, original,
        "uninstall leaves the preexisting foreign entry intact"
    );
    Ok(())
}

#[test]
fn indistinguishable_duplicate_entries_are_not_removed() -> TestResult {
    let mut store = tests::memory(tests::path("A", StringKind::Plain));
    tests::install(&mut store)?;
    store.state.path = tests::path(&format!("{ENTRY};F;{ENTRY}"), StringKind::Plain);
    let before = store.state.clone();
    assert_eq!(
        tests::remove(&mut store, PRODUCT),
        Err(Fault::AmbiguousLease),
        "two indistinguishable copies require explicit conflict handling"
    );
    assert_eq!(
        store.state, before,
        "ambiguity does not mutate PATH or ownership"
    );
    Ok(())
}

#[test]
fn invalid_raw_or_lease_bytes_fail_before_wal_or_path_write() {
    let mut store = tests::memory(RawPath::Present {
        kind: StringKind::Plain,
        bytes: vec![1],
    });
    let before = store.state.clone();
    assert_eq!(
        tests::install(&mut store),
        Err(Fault::InvalidRawPath),
        "an odd UTF16 byte count is not guessed"
    );
    assert_eq!(store.state, before, "invalid source is untouched");
    assert_eq!(
        Lease::try_new(tests::bytes("a;b")),
        Err(Fault::InvalidLease),
        "one lease cannot insert several elements"
    );
    assert_eq!(
        Lease::try_new(Vec::new()),
        Err(Fault::InvalidLease),
        "an empty lease is not a PATH element"
    );
}

#[test]
fn direct_remove_is_not_a_snapshot_restore_after_foreign_change() -> TestResult {
    let owner = tests::lease()?;
    let proof = path_edit::insert(&tests::path("A", StringKind::Plain), &owner)?;
    let current = tests::path(&format!("{ENTRY};NEW;A;"), StringKind::Expandable);
    assert_eq!(
        path_edit::remove(&current, &owner, &proof)?,
        tests::path("NEW;A;", StringKind::Expandable),
        "external type and source spelling stay authoritative"
    );
    Ok(())
}
