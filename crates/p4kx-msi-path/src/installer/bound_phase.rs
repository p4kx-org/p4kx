// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::arguments::{Invocation, Stage};
use crate::installer::binding::Binding;
use crate::installer::caller::Caller;
use crate::installer::fault::Fault;
use crate::installer::reservation;
use crate::installer::state::{PathChange, Phase, Snapshot, Store};
use crate::installer::transaction;

pub(crate) fn execute<S: Store>(
    store: &mut S,
    invocation: Invocation,
) -> Result<PathChange, S::Error> {
    let observed = store.load()?;
    let binding = invocation.binding;
    if observed.state.reservation.is_some() {
        reservation::matching(&observed.state, binding)?;
        return reserved(store, invocation);
    }
    if let Some(cancelled) = observed.state.cancelled_binding {
        if cancelled != binding {
            return Err(Fault::WrongTransaction.into());
        }
        return if matches!(invocation.stage, Stage::Cancel | Stage::Rollback) {
            Ok(PathChange::Unchanged)
        } else {
            Err(Fault::WrongPhase.into())
        };
    }
    if matches!(invocation.stage, Stage::Cancel | Stage::Preflight) {
        return Err(Fault::MissingReservation.into());
    }
    let caller = journal_caller(&observed.state, binding)?;
    let id = binding
        .sequence
        .checked_add(1)
        .ok_or(Fault::SequenceExhausted)?;
    match invocation.stage {
        Stage::Prepare => {
            if observed
                .state
                .journal
                .as_ref()
                .is_some_and(|record| record.phase == Phase::RolledBack)
            {
                return Err(Fault::WrongPhase.into());
            }
            Ok(PathChange::Unchanged)
        }
        Stage::Apply => transaction::apply(store, id, &caller),
        Stage::Commit => {
            transaction::commit(store, id, &caller)?;
            Ok(PathChange::Unchanged)
        }
        Stage::Rollback => transaction::rollback(store, id, &caller),
        Stage::Cancel | Stage::Preflight => Err(Fault::MissingReservation.into()),
    }
}

fn reserved<S: Store>(store: &mut S, invocation: Invocation) -> Result<PathChange, S::Error> {
    match invocation.stage {
        Stage::Preflight => {
            let observed = store.load()?;
            reservation::matching(&observed.state, invocation.binding)?.caller()?;
            Ok(PathChange::Unchanged)
        }
        Stage::Prepare => {
            transaction::begin_reserved(store, invocation.binding)?;
            Ok(PathChange::Unchanged)
        }
        Stage::Rollback | Stage::Cancel => {
            reservation::cancel(store, invocation.binding)?;
            Ok(PathChange::Unchanged)
        }
        Stage::Apply | Stage::Commit => Err(Fault::WrongPhase.into()),
    }
}

fn journal_caller(snapshot: &Snapshot, binding: Binding) -> Result<Caller, Fault> {
    let id = binding
        .sequence
        .checked_add(1)
        .ok_or(Fault::SequenceExhausted)?;
    let record = snapshot
        .journal
        .as_ref()
        .filter(|record| {
            record.id == id
                && snapshot.sequence == id
                && Binding::for_caller(binding.sequence, &record.caller) == binding
        })
        .ok_or(Fault::WrongTransaction)?;
    Ok(record.caller.clone())
}
