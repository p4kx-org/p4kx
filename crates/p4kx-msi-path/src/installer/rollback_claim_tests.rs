// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::claim::Claim;
use crate::installer::fault::Fault;
use crate::installer::journal_codec;
use crate::installer::lease::Lease;
use crate::installer::raw_path::{RawPath, StringKind};
use crate::installer::state::Request;
use crate::installer::tests::{
    self, ENTRY, Memory, NEXT_PRODUCT, PRODUCT, TestResult, UPGRADED_ENTRY,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Copies {
    Unique,
    Duplicate,
}

fn recreated(copies: Copies) -> String {
    match copies {
        Copies::Unique => format!("F;{ENTRY};A;;B;"),
        Copies::Duplicate => format!("F;{ENTRY};{ENTRY};A;;B;"),
    }
}

fn verify(mut store: Memory, id: u64, copies: Copies, expected: &RawPath) -> TestResult {
    let before = store.state.clone();
    let outcome = tests::rollback(&mut store, id);
    if copies == Copies::Duplicate {
        assert_eq!(
            outcome,
            Err(Fault::AmbiguousLease),
            "several foreign copies do not grant an arbitrary ownership claim"
        );
        assert_eq!(
            store.state, before,
            "failed compensation publishes no partial splice or owner"
        );
        return Ok(());
    }
    outcome?;
    assert_eq!(
        &store.state.path, expected,
        "compensation preserves the foreign spelling and Registry kind"
    );
    assert!(
        matches!(store.state.owner.as_ref(), Some(owner) if owner.product == PRODUCT && owner.claim == Claim::Borrowed),
        "the recreated foreign entry is borrowed rather than claimed as our insertion"
    );
    let encoded = journal_codec::encode(&store.state)?;
    assert_eq!(
        journal_codec::decode(&encoded)?,
        store.state,
        "durable recovery retains the new borrowed claim"
    );
    tests::remove(&mut store, PRODUCT)?;
    assert_eq!(
        &store.state.path, expected,
        "subsequent uninstall cannot remove a foreign recreated entry"
    );
    Ok(())
}

#[test]
fn uninstall_rollback_borrows_a_unique_foreign_recreated_lease_and_refuses_duplicates() -> TestResult
{
    for copies in [Copies::Unique, Copies::Duplicate] {
        let mut store = tests::memory(tests::path("A;B", StringKind::Expandable));
        tests::install(&mut store)?;
        let id = tests::begin(&mut store, Request::Remove { product: PRODUCT })?;
        tests::apply(&mut store, id)?;
        let expected = tests::path(&recreated(copies), StringKind::Plain);
        store.state.path = expected.clone();
        verify(store, id, copies, &expected)?;
    }
    Ok(())
}

#[test]
fn rename_rollback_removes_only_its_new_lease_and_borrows_the_foreign_old_lease() -> TestResult {
    for copies in [Copies::Unique, Copies::Duplicate] {
        let mut store = tests::memory(tests::path("A;B", StringKind::Expandable));
        tests::install(&mut store)?;
        let id = tests::begin(
            &mut store,
            Request::Upgrade {
                from: PRODUCT,
                to: NEXT_PRODUCT,
                lease: Lease::try_new(tests::bytes(UPGRADED_ENTRY))?,
            },
        )?;
        tests::apply(&mut store, id)?;
        let foreign = recreated(copies);
        store.state.path = tests::path(&format!("{UPGRADED_ENTRY};{foreign}"), StringKind::Plain);
        let expected = tests::path(&foreign, StringKind::Plain);
        verify(store, id, copies, &expected)?;
    }
    Ok(())
}
