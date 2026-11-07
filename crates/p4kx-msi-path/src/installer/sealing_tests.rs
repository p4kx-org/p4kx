// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::arguments::{Invocation, Stage};
use crate::installer::binding::Binding;
use crate::installer::bound_phase;
use crate::installer::declaration::Declaration;
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;
use crate::installer::journal_codec;
use crate::installer::raw_path::StringKind;
use crate::installer::reservation::{self, Plan, Reservation};
use crate::installer::reservation_identity::Identity;
use crate::installer::tests::{self, Memory, TestResult};
use crate::installer::transaction;

pub(super) fn identity(byte: u8) -> Result<Identity, Fault> {
    Ok(Identity {
        product: 1,
        package: 101,
        nonce: InvocationNonce::from_bytes([byte; 32])?,
    })
}

pub(super) fn declaration() -> Result<Declaration, Fault> {
    let caller = tests::reservation::caller(9)?;
    Ok(Declaration {
        product: caller.product,
        package: caller.package,
        predecessor: caller.predecessor,
        goal: caller.goal,
    })
}

pub(super) fn neutral() -> Result<(Memory, Binding), Fault> {
    let mut memory = tests::memory(tests::path(";;A;;", StringKind::Expandable));
    let binding = reservation::create(&mut memory, identity(9)?)?;
    Ok((memory, binding))
}

#[test]
fn early_reserve_retains_only_identity_and_preserves_terminal_owner_and_raw_path() -> TestResult {
    let mut memory = tests::memory(tests::path(";;A;;", StringKind::Expandable));
    memory.state = tests::installed(memory.state.path.clone())?;
    let before = memory.state.clone();
    let binding = reservation::create(&mut memory, identity(9)?)?;
    let reserved = memory
        .state
        .reservation
        .as_ref()
        .ok_or("missing neutral reservation")?;
    assert!(
        matches!(reserved.plan, Plan::Neutral(_)),
        "early reserve invents no goal or resolved folder"
    );
    assert_eq!(
        reserved.caller(),
        Err(Fault::WrongPhase),
        "neutral identity is not a Caller"
    );
    assert_eq!(
        binding.sequence, before.sequence,
        "reserve consumes no receipt"
    );
    assert_eq!(memory.state.path, before.path, "raw PATH is not changed");
    assert_eq!(
        memory.state.owner, before.owner,
        "complete ownership remains pinned"
    );
    assert_eq!(
        memory.state.journal, before.journal,
        "terminal WAL remains pinned"
    );
    assert_eq!(
        journal_codec::decode(&journal_codec::encode(&memory.state)?)?,
        memory.state,
        "neutral state has an explicit durable representation"
    );
    Ok(())
}

#[test]
fn neutral_cannot_prepare_apply_commit_or_preflight_and_noop_cancel_only_fences_cleanup()
-> TestResult {
    let (mut memory, binding) = neutral()?;
    let before = memory.state.clone();
    for stage in [
        Stage::Prepare,
        Stage::Apply,
        Stage::Commit,
        Stage::Preflight,
    ] {
        assert_eq!(
            bound_phase::execute(&mut memory, Invocation { stage, binding }),
            Err(Fault::WrongPhase),
            "costing-dependent intent is required"
        );
        assert_eq!(memory.state, before, "refusal changes no durable field");
    }
    reservation::cancel(&mut memory, binding)?;
    let cancelled = memory.state.clone();
    for stage in [Stage::Rollback, Stage::Cancel] {
        bound_phase::execute(&mut memory, Invocation { stage, binding })?;
    }
    assert_eq!(
        reservation::seal(&mut memory, binding, declaration()?),
        Err(Fault::MissingReservation),
        "noop cancellation cannot be resealed"
    );
    assert_eq!(
        memory.state, cancelled,
        "exact cleanup repeats preserve the fence"
    );
    assert_eq!(
        memory.state.path, before.path,
        "feature noop has no PATH effect"
    );
    assert_eq!(
        memory.state.sequence, before.sequence,
        "feature noop has no receipt"
    );
    Ok(())
}

#[test]
fn seal_is_once_only_and_prepare_snapshots_the_later_foreign_path() -> TestResult {
    let (mut memory, binding) = neutral()?;
    memory.state.path = tests::path("foreign-before-seal;;A;", StringKind::Expandable);
    let before = memory.state.clone();
    reservation::seal(&mut memory, binding, declaration()?)?;
    assert_eq!(
        memory.state.path, before.path,
        "seal does not take an obsolete PATH snapshot"
    );
    assert_eq!(
        memory.state.owner, before.owner,
        "seal leaves ownership untouched"
    );
    assert_eq!(
        memory.state.journal, before.journal,
        "seal creates no terminal receipt"
    );
    assert_eq!(
        memory.state.reservation.as_ref().map(Reservation::binding),
        Some(binding),
        "seal never replaces the captured nonce or N"
    );
    let sealed = memory.state.clone();
    assert_eq!(
        reservation::seal(&mut memory, binding, declaration()?),
        Err(Fault::WrongPhase),
        "even identical second seal cannot rewrite intent"
    );
    assert_eq!(memory.state, sealed, "repeated seal is immutable");
    let live = tests::path("foreign-after-seal;;A;", StringKind::Expandable);
    memory.state.path = live.clone();
    transaction::begin_reserved(&mut memory, binding)?;
    assert_eq!(
        memory
            .state
            .journal
            .as_ref()
            .map(|record| &record.before_path),
        Some(&live),
        "only prepare reads the live PATH"
    );
    Ok(())
}

#[test]
fn seal_validates_exact_captured_identity_and_the_fresh_predecessor() -> TestResult {
    let mut memory = tests::memory(tests::path("A", StringKind::Plain));
    memory.state = tests::installed(memory.state.path.clone())?;
    let binding = reservation::create(
        &mut memory,
        Identity {
            product: 2,
            package: 202,
            ..identity(9)?
        },
    )?;
    let before = memory.state.clone();
    assert_eq!(
        reservation::seal(&mut memory, binding, declaration()?),
        Err(Fault::InvalidIdentity),
        "actual MSI identity cannot be replaced by a plan"
    );
    let mut next = declaration()?;
    next.product = 2;
    next.package = 202;
    assert_eq!(
        reservation::seal(&mut memory, binding, next.clone()),
        Err(Fault::WrongOwner),
        "another owner requires the fresh predecessor"
    );
    assert_eq!(
        memory.state, before,
        "bad intent cannot mutate neutral authority"
    );
    next.predecessor = Some(1);
    reservation::seal(&mut memory, binding, next)?;
    assert_eq!(
        memory.state.owner, before.owner,
        "sealing an upgrade does not transfer ownership early"
    );
    Ok(())
}

#[test]
fn explicit_recovery_preserves_both_reservation_phases_and_invalidates_stale_seal() -> TestResult {
    for sealed in [false, true] {
        let (mut memory, binding) = neutral()?;
        if sealed {
            reservation::seal(&mut memory, binding, declaration()?)?;
        }
        let mut expected = memory.state.clone();
        let nonce = InvocationNonce::from_bytes([10; 32])?;
        expected
            .reservation
            .as_mut()
            .ok_or("missing reservation")?
            .set_nonce(nonce);
        let replacement = reservation::recover(&mut memory, binding, nonce)?;
        assert_eq!(
            memory.state, expected,
            "only the nonce is replaced, including the exact sealed intent"
        );
        assert_eq!(
            reservation::seal(&mut memory, binding, declaration()?),
            Err(Fault::WrongTransaction),
            "old captured token loses all seal authority"
        );
        assert_eq!(
            memory.state, expected,
            "stale seal cannot modify recovered authority"
        );
        if !sealed {
            reservation::seal(&mut memory, replacement, declaration()?)?;
        }
        transaction::begin_reserved(&mut memory, replacement)?;
    }
    Ok(())
}

#[test]
fn wal4_rejects_v3_unknown_plan_tags_and_every_truncation_in_both_phases() -> TestResult {
    let (mut memory, binding) = neutral()?;
    for sealed in [false, true] {
        if sealed {
            reservation::seal(&mut memory, binding, declaration()?)?;
        }
        let bytes = journal_codec::encode(&memory.state)?;
        for end in 0..bytes.len() {
            assert!(
                journal_codec::decode(bytes.get(..end).ok_or("invalid truncation")?).is_err(),
                "a partial reservation cannot become authority"
            );
        }
        for version in 1..=3 {
            let mut old = bytes.clone();
            *old.get_mut(journal_codec::MAGIC.len() - 1)
                .ok_or("missing version")? = version;
            assert_eq!(
                journal_codec::decode(&old),
                Err(Fault::CorruptState),
                "incompatible WAL bytes cannot become two-stage WAL4 authority"
            );
        }
        if !sealed {
            let mut corrupt = bytes;
            let offset = corrupt.len().checked_sub(2).ok_or("missing plan tag")?;
            *corrupt.get_mut(offset).ok_or("missing neutral plan tag")? = 255;
            assert_eq!(
                journal_codec::decode(&corrupt),
                Err(Fault::CorruptState),
                "unknown plan tags are never treated as Neutral"
            );
        }
    }
    Ok(())
}

#[test]
fn neutral_reservation_blocks_every_terminal_mutator_without_changing_pinned_evidence() -> TestResult
{
    let mut memory = tests::memory(tests::path("A;;", StringKind::Expandable));
    memory.state = tests::installed(memory.state.path.clone())?;
    let record = memory
        .state
        .journal
        .as_ref()
        .ok_or("missing terminal WAL")?
        .clone();
    reservation::create(&mut memory, identity(9)?)?;
    let expected = memory.state.clone();
    assert_eq!(
        transaction::apply(&mut memory, record.id, &record.caller),
        Err(Fault::PendingReservation),
        "old apply noop cannot cross Neutral"
    );
    assert_eq!(
        transaction::commit(&mut memory, record.id, &record.caller),
        Err(Fault::PendingReservation),
        "old commit noop cannot cross Neutral"
    );
    assert_eq!(
        transaction::rollback(&mut memory, record.id, &record.caller),
        Err(Fault::PendingReservation),
        "old rollback cannot change the reserved owner or terminal WAL"
    );
    assert_eq!(
        memory.state, expected,
        "full owner, WAL, N and PATH remain unchanged"
    );
    Ok(())
}
