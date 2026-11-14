// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;
use crate::installer::raw_path::{RawPath, StringKind};
use crate::installer::state::Request;

use super::{PRODUCT, TestResult};
use crate::installer::tests;

#[test]
fn a_rollback_before_begin_cannot_undo_a_previous_committed_install() -> TestResult {
    let mut store = tests::memory(tests::path(";A;;", StringKind::Expandable));
    tests::install(&mut store)?;
    let previous = store.state.clone();
    tests::rollback_at(&mut store, previous.sequence)?;
    assert_eq!(
        store.state, previous,
        "captured expected sequence distinguishes this invocation from the prior receipt"
    );
    Ok(())
}

#[test]
fn stale_preflight_receipt_fails_before_any_wal_or_path_write() -> TestResult {
    let mut store = tests::memory(tests::path("A;B", StringKind::Plain));
    tests::install(&mut store)?;
    let current = store.state.clone();
    let result = tests::begin_at(&mut store, 0, Request::Repair { product: PRODUCT });
    assert_eq!(
        result,
        Err(Fault::Conflict),
        "stale AppSearch state is not an authority to overwrite current state"
    );
    assert_eq!(store.state, current, "stale invocation mutates nothing");
    assert_eq!(
        tests::rollback_at(&mut store, u64::MAX),
        Err(Fault::SequenceExhausted),
        "overflow cannot wrap onto another receipt"
    );
    Ok(())
}

#[test]
fn matching_preflight_receipt_can_roll_back_prepared_applied_and_committed_cuts() -> TestResult {
    for cut in 0..3 {
        let original = tests::path(";;A;;B;", StringKind::Expandable);
        let mut store = tests::memory(original.clone());
        let id = tests::begin(
            &mut store,
            Request::Install {
                product: PRODUCT,
                lease: tests::lease()?,
            },
        )?;
        if cut >= 1 {
            tests::apply(&mut store, id)?;
        }
        if cut >= 2 {
            tests::commit(&mut store, id)?;
        }
        tests::rollback_at(&mut store, 0)?;
        tests::rollback_at(&mut store, 0)?;
        assert_eq!(
            store.state.path, original,
            "same callback receipt restores the matching transaction only"
        );
    }
    Ok(())
}

#[test]
fn pending_or_wrong_receipt_is_explicit_and_never_guessed() -> TestResult {
    let mut store = tests::memory(RawPath::Absent);
    let id = tests::begin(
        &mut store,
        Request::Install {
            product: PRODUCT,
            lease: tests::lease()?,
        },
    )?;
    assert_eq!(
        tests::begin(&mut store, Request::Remove { product: PRODUCT }),
        Err(Fault::PendingTransaction),
        "unknown previous MSI outcome needs explicit recovery"
    );
    assert_eq!(
        tests::apply(&mut store, id + 1),
        Err(Fault::WrongTransaction),
        "a foreign callback cannot own this WAL"
    );
    assert_eq!(
        tests::commit(&mut store, id),
        Err(Fault::WrongPhase),
        "prepare cannot masquerade as apply"
    );
    tests::rollback(&mut store, id)?;
    assert_eq!(
        tests::apply(&mut store, id),
        Err(Fault::WrongPhase),
        "a rolled-back operation cannot later apply"
    );
    Ok(())
}

#[test]
fn atomic_compare_failure_never_overwrites_interleaved_foreign_changes() {
    let mut store = tests::memory(tests::path("A", StringKind::Plain));
    let foreign = tests::path("OTHER;A", StringKind::Expandable);
    store.interleave = Some(foreign.clone());
    assert_eq!(
        tests::install(&mut store),
        Err(Fault::Conflict),
        "prepare compares the entire current state atomically"
    );
    assert_eq!(
        store.state.path, foreign,
        "the unrelated writer wins without lost update"
    );
    assert!(
        store.state.journal.is_none(),
        "no half journal was published"
    );
}

#[test]
fn apply_conflict_keeps_wal_and_preserves_foreign_state_during_recovery() -> TestResult {
    let mut store = tests::memory(tests::path("A", StringKind::Plain));
    let id = tests::begin(
        &mut store,
        Request::Install {
            product: PRODUCT,
            lease: tests::lease()?,
        },
    )?;
    let foreign = tests::path("FOREIGN;A", StringKind::Plain);
    store.interleave = Some(foreign.clone());
    assert_eq!(
        tests::apply(&mut store, id),
        Err(Fault::Conflict),
        "apply and its phase marker are atomic"
    );
    tests::rollback(&mut store, id)?;
    assert_eq!(
        store.state.path, foreign,
        "prepared WAL cleanup cannot erase a foreign write"
    );
    Ok(())
}

#[test]
fn exhausted_sequence_fails_without_mutation() {
    let mut store = tests::memory(RawPath::Absent);
    store.state.sequence = u64::MAX;
    let before = store.state.clone();
    assert_eq!(
        tests::install(&mut store),
        Err(Fault::SequenceExhausted),
        "WAL sequence cannot wrap into an old receipt"
    );
    assert_eq!(store.state, before, "overflow does not acquire ownership");
}
