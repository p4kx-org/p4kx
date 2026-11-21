// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::arguments::{Invocation, Stage};
use crate::installer::binding::Binding;
use crate::installer::bound_phase;
use crate::installer::caller::{Caller, Goal};
use crate::installer::declaration::Declaration;
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;
use crate::installer::journal_codec;
use crate::installer::raw_path::StringKind;
use crate::installer::reservation;
use crate::installer::reservation_identity::Identity;
use crate::installer::state::Store;
use crate::installer::state::{Phase, Request};
use crate::installer::tests::{self, Memory, TestResult};
use crate::installer::transaction;

pub(super) fn caller(byte: u8) -> Result<Caller, Fault> {
    Ok(Caller {
        product: 1,
        package: 101,
        nonce: InvocationNonce::from_bytes([byte; 32])?,
        predecessor: None,
        goal: Goal::Ensure(tests::lease()?),
    })
}

pub(super) fn reserve<S: Store>(store: &mut S, caller: Caller) -> Result<Binding, S::Error> {
    let identity = Identity {
        product: caller.product,
        package: caller.package,
        nonce: caller.nonce,
    };
    let binding = reservation::create(store, identity)?;
    reservation::seal(
        store,
        binding,
        Declaration {
            product: caller.product,
            package: caller.package,
            predecessor: caller.predecessor,
            goal: caller.goal,
        },
    )?;
    Ok(binding)
}

pub(super) fn reserved() -> Result<(Memory, Binding), Fault> {
    let mut memory = tests::memory(tests::path(";A;;", StringKind::Expandable));
    let binding = reserve(&mut memory, caller(9)?)?;
    Ok((memory, binding))
}

#[test]
fn reservation_keeps_the_complete_owner_terminal_wal_sequence_and_path_unchanged() -> TestResult {
    let mut memory = tests::memory(tests::path(";;A;;", StringKind::Expandable));
    memory.state = tests::installed(memory.state.path.clone())?;
    let before = memory.state.clone();
    let binding = reserve(&mut memory, caller(9)?)?;
    assert_eq!(
        binding.sequence, before.sequence,
        "reserve does not advance the durable receipt"
    );
    assert_eq!(
        memory.state.path, before.path,
        "reserve does not snapshot or modify the eventual prepare input"
    );
    assert_eq!(
        memory.state.owner, before.owner,
        "the full Borrowed/Inserted claim remains pinned"
    );
    assert_eq!(
        memory.state.journal, before.journal,
        "terminal receipt remains byte-for-byte representable"
    );
    assert_eq!(
        journal_codec::decode(&journal_codec::encode(&memory.state)?)?,
        memory.state,
        "fully decoded reservation survives durable reload"
    );
    Ok(())
}

#[test]
fn prepare_uses_the_fresh_foreign_path_and_atomically_consumes_the_reservation() -> TestResult {
    let (mut memory, binding) = reserved()?;
    let fresh = tests::path(";foreign;;A;", StringKind::Expandable);
    memory.state.path = fresh.clone();
    let id = transaction::begin_reserved(&mut memory, binding)?;
    assert!(
        memory.state.reservation.is_none(),
        "prepare consumes the authority in the same replacement as its WAL"
    );
    let record = memory
        .state
        .journal
        .as_ref()
        .ok_or("missing prepared WAL")?;
    assert_eq!(
        record.before_path, fresh,
        "historical cleanup and foreign edits are observed only at actual prepare"
    );
    let caller = record.caller.clone();
    transaction::apply(&mut memory, id, &caller)?;
    transaction::rollback(&mut memory, id, &caller)?;
    assert_eq!(
        memory.state.path, fresh,
        "rollback restores the actual prepare input, not the earlier reserved PATH"
    );
    Ok(())
}

#[test]
fn default_collision_and_foreign_cancel_never_replace_the_original_reservation() -> TestResult {
    let (mut memory, binding) = reserved()?;
    let before = memory.state.clone();
    assert_eq!(
        reserve(&mut memory, caller(10)?),
        Err(Fault::PendingReservation),
        "a new invocation must not adopt or overwrite a pending reservation"
    );
    let foreign = Binding {
        nonce: InvocationNonce::from_bytes([10; 32])?,
        ..binding
    };
    assert_eq!(
        reservation::cancel(&mut memory, foreign),
        Err(Fault::WrongTransaction),
        "Product and Package alone cannot cancel"
    );
    assert_eq!(
        memory.state, before,
        "refused callers leave every field unchanged"
    );
    reservation::cancel(&mut memory, binding)?;
    assert!(
        memory.state.reservation.is_none(),
        "only the exact reserved binding is released"
    );
    assert_eq!(
        memory.state.sequence, binding.sequence,
        "cancelling metadata consumes no PATH receipt"
    );
    Ok(())
}

#[test]
fn explicit_recovery_changes_only_nonce_and_rejects_every_old_callback() -> TestResult {
    let (mut memory, binding) = reserved()?;
    let mut expected = memory.state.clone();
    let nonce = InvocationNonce::from_bytes([10; 32])?;
    expected
        .reservation
        .as_mut()
        .ok_or("missing reservation")?
        .set_nonce(nonce);
    let recovered = reservation::recover(&mut memory, binding, nonce)?;
    assert_eq!(
        memory.state, expected,
        "recovery preserves Product/Package/Goal/predecessor/N/owner/WAL/PATH"
    );
    for stage in [
        Stage::Preflight,
        Stage::Prepare,
        Stage::Apply,
        Stage::Commit,
        Stage::Rollback,
        Stage::Cancel,
    ] {
        assert_eq!(
            bound_phase::execute(&mut memory, Invocation { stage, binding }),
            Err(Fault::WrongTransaction),
            "old captured callbacks cannot cross the recovery boundary"
        );
    }
    assert_eq!(
        memory.state, expected,
        "stale callbacks cannot erase the replacement authority"
    );
    transaction::begin_reserved(&mut memory, recovered)?;
    assert!(
        memory.state.reservation.is_none(),
        "the fresh invocation retains its legitimate prepare"
    );
    Ok(())
}

#[test]
fn prepared_applied_committed_and_rolled_back_receipts_are_not_recoverable_reservations()
-> TestResult {
    for phase in [
        Phase::Prepared,
        Phase::Applied,
        Phase::Committed,
        Phase::RolledBack,
    ] {
        let mut memory = tests::memory(tests::path("A", StringKind::Plain));
        memory.state = tests::installation_at(memory.state.path.clone(), phase)?;
        let record = memory.state.journal.as_ref().ok_or("missing journal")?;
        let binding = Binding::for_caller(0, &record.caller);
        let before = memory.state.clone();
        assert_eq!(
            reservation::recover(&mut memory, binding, InvocationNonce::from_bytes([99; 32])?),
            Err(Fault::MissingReservation),
            "recovery never treats any WAL phase as reserved-only metadata"
        );
        assert_eq!(
            memory.state, before,
            "existing receipts are retained exactly"
        );
    }
    Ok(())
}

#[test]
fn every_legacy_mutator_is_blocked_while_another_invocation_is_reserved() -> TestResult {
    let mut memory = tests::memory(tests::path("A", StringKind::Plain));
    memory.state = tests::installed(memory.state.path.clone())?;
    let record = memory.state.journal.as_ref().ok_or("missing journal")?;
    let old = record.caller.clone();
    let id = record.id;
    reserve(&mut memory, caller(99)?)?;
    let before = memory.state.clone();
    assert_eq!(
        transaction::apply(&mut memory, id, &old),
        Err(Fault::PendingReservation),
        "old applied no-op cannot adopt authority"
    );
    assert_eq!(
        transaction::commit(&mut memory, id, &old),
        Err(Fault::PendingReservation),
        "old commit cannot cross reservation"
    );
    assert_eq!(
        transaction::rollback(&mut memory, id, &old),
        Err(Fault::PendingReservation),
        "old rollback cannot change the pinned owner"
    );
    assert_eq!(
        transaction::rollback_at(&mut memory, id, &old),
        Err(Fault::PendingReservation),
        "early rollback no-op is guarded"
    );
    assert_eq!(
        transaction::preflight(&memory, id, &old),
        Err(Fault::PendingReservation),
        "old preflight does not validate another invocation"
    );
    assert_eq!(
        transaction::begin_at(&mut memory, id, &old, Request::Repair { product: 1 }),
        Err(Fault::PendingReservation),
        "unbound prepare cannot bypass reservation"
    );
    assert_eq!(
        memory.state, before,
        "all old callbacks preserve the complete pinned source state"
    );
    Ok(())
}

#[test]
fn repeated_nonces_and_invalid_reservation_codec_states_fail_closed() -> TestResult {
    let (mut memory, binding) = reserved()?;
    let before = memory.state.clone();
    assert_eq!(
        reservation::recover(&mut memory, binding, binding.nonce),
        Err(Fault::InvalidNonce),
        "recovery requires a fresh nonce"
    );
    assert_eq!(
        memory.state, before,
        "failed recovery cannot mutate metadata"
    );
    let mut corrupt = before.clone();
    corrupt
        .reservation
        .as_mut()
        .ok_or("missing reservation")?
        .base_sequence = 1;
    assert_eq!(
        journal_codec::encode(&corrupt),
        Err(Fault::CorruptState),
        "base N must agree with the pinned state"
    );
    let mut old = journal_codec::encode(&before)?;
    *old.get_mut(journal_codec::MAGIC.len() - 1)
        .ok_or("missing version")? = 3;
    assert_eq!(
        journal_codec::decode(&old),
        Err(Fault::CorruptState),
        "WAL3 bytes cannot be reinterpreted as WAL4"
    );
    Ok(())
}
