// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::arguments::{Invocation, Stage};
use crate::installer::bound_phase;
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;
use crate::installer::journal_codec;
use crate::installer::reservation;
use crate::installer::state::Phase;
use crate::installer::tests::{self, TestResult};
use crate::installer::transaction;

#[test]
fn exact_cancel_and_rollback_repeats_are_noops_without_reactivating_other_phases() -> TestResult {
    let (mut memory, binding) = tests::reservation::reserved()?;
    let before = memory.state.clone();
    reservation::cancel(&mut memory, binding)?;
    let cancelled = memory.state.clone();
    assert!(
        cancelled.reservation.is_none(),
        "no active reservation remains"
    );
    assert_eq!(
        cancelled.cancelled_binding,
        Some(binding),
        "idempotence retains only the exact same binding"
    );
    assert_eq!(
        cancelled.path, before.path,
        "cancel never restores an earlier PATH snapshot"
    );
    assert_eq!(
        cancelled.owner, before.owner,
        "cancel preserves the full current owner"
    );
    assert_eq!(
        cancelled.journal, before.journal,
        "cancel preserves the terminal WAL"
    );
    assert_eq!(
        cancelled.sequence, before.sequence,
        "cancel does not consume a receipt"
    );
    for stage in [Stage::Cancel, Stage::Rollback] {
        bound_phase::execute(&mut memory, Invocation { stage, binding })?;
    }
    for stage in [
        Stage::Preflight,
        Stage::Prepare,
        Stage::Apply,
        Stage::Commit,
    ] {
        assert_eq!(
            bound_phase::execute(&mut memory, Invocation { stage, binding }),
            Err(Fault::WrongPhase),
            "cancelled binding authorises cleanup only"
        );
    }
    assert_eq!(
        memory.state, cancelled,
        "cleanup repeats do not regain mutation authority"
    );
    assert_eq!(
        journal_codec::decode(&journal_codec::encode(&cancelled)?)?,
        cancelled,
        "the exact cleanup fence is durable"
    );
    Ok(())
}

#[test]
fn a_new_reservation_atomically_replaces_the_cancel_fence_and_rejects_old_cleanup() -> TestResult {
    let (mut memory, binding) = tests::reservation::reserved()?;
    reservation::cancel(&mut memory, binding)?;
    assert_eq!(
        reservation::create(&mut memory, tests::reservation::caller(9)?),
        Err(Fault::InvalidNonce),
        "the cancelled invocation nonce is not fresh"
    );
    let fresh = reservation::create(&mut memory, tests::reservation::caller(10)?)?;
    let before = memory.state.clone();
    assert!(
        before.cancelled_binding.is_none(),
        "new authority removes the old cleanup-only fence atomically"
    );
    for stage in [Stage::Cancel, Stage::Rollback] {
        assert_eq!(
            bound_phase::execute(&mut memory, Invocation { stage, binding }),
            Err(Fault::WrongTransaction),
            "old cleanup cannot cancel a later invocation at the same N"
        );
    }
    assert_eq!(
        memory.state, before,
        "fresh reservation is retained exactly"
    );
    transaction::begin_reserved(&mut memory, fresh)?;
    Ok(())
}

#[test]
fn cancelling_retains_foreign_path_edits_and_still_fences_old_terminal_callbacks() -> TestResult {
    let (mut memory, _) = tests::reservation::reserved()?;
    memory.state.reservation = None;
    memory.state = tests::installed(memory.state.path.clone())?;
    let old = memory
        .state
        .journal
        .as_ref()
        .ok_or("missing old WAL")?
        .clone();
    let binding = reservation::create(&mut memory, tests::reservation::caller(10)?)?;
    let foreign = tests::path(
        "foreign;;A;",
        crate::installer::raw_path::StringKind::Expandable,
    );
    memory.state.path = foreign.clone();
    reservation::cancel(&mut memory, binding)?;
    assert_eq!(
        memory.state.path, foreign,
        "reserved-only cleanup cannot erase foreign changes"
    );
    assert_eq!(
        transaction::commit(&mut memory, old.id, &old.caller),
        Err(Fault::PendingReservation),
        "old committed authority does not return after a later cancel"
    );
    assert_eq!(
        transaction::rollback(&mut memory, old.id, &old.caller),
        Err(Fault::PendingReservation),
        "old rollback cannot change the pinned owner after a cancel"
    );
    assert_eq!(
        reservation::recover(&mut memory, binding, InvocationNonce::from_bytes([11; 32])?),
        Err(Fault::MissingReservation),
        "cancelled fences are not recoverable active reservations"
    );
    Ok(())
}

#[test]
fn malformed_or_nonterminal_cancellation_fences_are_rejected_by_the_codec() -> TestResult {
    let (mut memory, binding) = tests::reservation::reserved()?;
    memory.state.cancelled_binding = Some(binding);
    assert_eq!(
        journal_codec::encode(&memory.state),
        Err(Fault::CorruptState),
        "active and cancelled authority are mutually exclusive"
    );
    memory.state.reservation = None;
    let valid = memory.state.clone();
    for changed in [
        crate::installer::binding::Binding {
            sequence: 1,
            ..binding
        },
        crate::installer::binding::Binding {
            product: 0,
            ..binding
        },
        crate::installer::binding::Binding {
            package: 0,
            ..binding
        },
    ] {
        memory.state.cancelled_binding = Some(changed);
        assert!(
            journal_codec::encode(&memory.state).is_err(),
            "N and both nonzero MSI identities must remain canonical"
        );
    }
    for phase in [Phase::Prepared, Phase::Applied] {
        memory.state = tests::installation_at(valid.path.clone(), phase)?;
        memory.state.cancelled_binding = Some(crate::installer::binding::Binding {
            sequence: memory.state.sequence,
            ..binding
        });
        assert_eq!(
            journal_codec::encode(&memory.state),
            Err(Fault::PendingTransaction),
            "a cancellation fence cannot coexist with a materialised pending WAL"
        );
    }
    Ok(())
}
