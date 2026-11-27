// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;
use crate::installer::raw_path::StringKind;
use crate::installer::state::{Phase, Request};

use super::{ENTRY, NEXT_PRODUCT, PRODUCT, TestResult};
use crate::installer::tests;

#[test]
fn a_prepared_crash_can_roll_back_without_erasing_later_foreign_edits() -> TestResult {
    let mut store = tests::memory(tests::path("A", StringKind::Plain));
    let id = tests::begin(
        &mut store,
        Request::Install {
            product: PRODUCT,
            lease: tests::lease()?,
        },
    )?;
    store.state.path = tests::path("FOREIGN;A", StringKind::Expandable);
    tests::rollback(&mut store, id)?;
    assert_eq!(
        store.state.path,
        tests::path("FOREIGN;A", StringKind::Expandable),
        "prepared WAL never owned a PATH mutation"
    );
    assert_eq!(
        store.state.journal.as_ref().ok_or("journal")?.phase,
        Phase::RolledBack,
        "recovery is durable"
    );
    Ok(())
}

#[test]
fn applied_crash_and_repeated_callbacks_are_idempotent() -> TestResult {
    let original = tests::path(";;A;;", StringKind::Expandable);
    let mut store = tests::memory(original.clone());
    let id = tests::begin(
        &mut store,
        Request::Install {
            product: PRODUCT,
            lease: tests::lease()?,
        },
    )?;
    tests::apply(&mut store, id)?;
    let applied = store.state.clone();
    tests::apply(&mut store, id)?;
    assert_eq!(
        store.state, applied,
        "retry does not insert another private entry"
    );
    tests::rollback(&mut store, id)?;
    tests::rollback(&mut store, id)?;
    assert_eq!(
        store.state.path, original,
        "a restarted callback restores the exact owned original"
    );
    Ok(())
}

#[test]
fn later_commit_action_failure_can_still_roll_back_our_commit_marker() -> TestResult {
    let original = tests::path("A;", StringKind::Expandable);
    let mut store = tests::memory(original.clone());
    let id = tests::install(&mut store)?;
    tests::commit(&mut store, id)?;
    tests::rollback(&mut store, id)?;
    assert_eq!(
        store.state.path, original,
        "commit marking does not destroy rollback evidence prematurely"
    );
    Ok(())
}

#[test]
fn foreign_edits_after_apply_survive_install_rollback() -> TestResult {
    let mut store = tests::memory(tests::path("A", StringKind::Plain));
    let id = tests::install(&mut store)?;
    store.state.path = tests::path(&format!("F;{ENTRY};A;G;"), StringKind::Expandable);
    tests::rollback(&mut store, id)?;
    assert_eq!(
        store.state.path,
        tests::path("F;A;G;", StringKind::Expandable),
        "rollback compensates only its exact owned insertion"
    );
    Ok(())
}

#[test]
fn repair_does_not_duplicate_and_reacquires_only_when_missing() -> TestResult {
    let mut store = tests::memory(tests::path("A", StringKind::Plain));
    tests::install(&mut store)?;
    let existing = store.state.path.clone();
    let id = tests::begin(&mut store, Request::Repair { product: PRODUCT })?;
    tests::apply(&mut store, id)?;
    tests::commit(&mut store, id)?;
    assert_eq!(
        store.state.path, existing,
        "repair of an owned entry is byte-idempotent"
    );
    store.state.path = tests::path("NEW;;A;", StringKind::Expandable);
    let id = tests::begin(&mut store, Request::Repair { product: PRODUCT })?;
    tests::apply(&mut store, id)?;
    tests::commit(&mut store, id)?;
    tests::remove(&mut store, PRODUCT)?;
    assert_eq!(
        store.state.path,
        tests::path("NEW;;A;", StringKind::Expandable),
        "repair records its actual reacquisition baseline"
    );
    Ok(())
}

#[test]
fn upgrade_transfers_ownership_without_rewriting_path_and_can_rollback() -> TestResult {
    let mut store = tests::memory(tests::path("A;", StringKind::Plain));
    tests::install(&mut store)?;
    let original = store.state.path.clone();
    let id = tests::begin(
        &mut store,
        Request::Transfer {
            from: PRODUCT,
            to: NEXT_PRODUCT,
        },
    )?;
    tests::apply(&mut store, id)?;
    assert_eq!(
        store.state.path, original,
        "upgrade does not acquire another PATH entry"
    );
    assert_eq!(
        store.state.owner.as_ref().ok_or("owner")?.product,
        NEXT_PRODUCT,
        "the new MSI owns the lease"
    );
    tests::rollback(&mut store, id)?;
    assert_eq!(
        store.state.owner.as_ref().ok_or("owner")?.product,
        PRODUCT,
        "rollback restores previous product ownership"
    );
    Ok(())
}

#[test]
fn old_product_cannot_remove_new_ownership_and_new_uninstall_preserves_original() -> TestResult {
    let original = tests::path(";A;;", StringKind::Expandable);
    let mut store = tests::memory(original.clone());
    tests::install(&mut store)?;
    let id = tests::begin(
        &mut store,
        Request::Transfer {
            from: PRODUCT,
            to: NEXT_PRODUCT,
        },
    )?;
    tests::apply(&mut store, id)?;
    tests::commit(&mut store, id)?;
    assert_eq!(
        tests::remove(&mut store, PRODUCT),
        Err(Fault::WrongOwner),
        "old MSI removal cannot tear down a transferred lease"
    );
    tests::remove(&mut store, NEXT_PRODUCT)?;
    assert_eq!(
        store.state.path, original,
        "upgrade retains the genuine initial ownership baseline"
    );
    Ok(())
}

#[test]
fn uninstall_rollback_reacquires_against_current_foreign_state() -> TestResult {
    let mut store = tests::memory(tests::path("A", StringKind::Plain));
    tests::install(&mut store)?;
    let id = tests::begin(&mut store, Request::Remove { product: PRODUCT })?;
    tests::apply(&mut store, id)?;
    store.state.path = tests::path("NEW;;A;", StringKind::Expandable);
    tests::rollback(&mut store, id)?;
    tests::remove(&mut store, PRODUCT)?;
    assert_eq!(
        store.state.path,
        tests::path("NEW;;A;", StringKind::Expandable),
        "compensation does not restore an obsolete full PATH snapshot"
    );
    Ok(())
}
