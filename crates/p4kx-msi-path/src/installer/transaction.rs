// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::binding::Binding;
use crate::installer::caller::Caller;
use crate::installer::claim;
use crate::installer::compensation;
use crate::installer::fault::Fault;
use crate::installer::lease::Lease;
use crate::installer::path_edit;
use crate::installer::raw_path::RawPath;
use crate::installer::reservation;
use crate::installer::rollback_receipt::{RollbackReceipt, RollbackSource};

use crate::installer::state::{
    Journal, Observed, Ownership, PathChange, Phase, Request, Snapshot, Store,
};

#[derive(Clone, Copy)]
struct Source<'a> {
    path: &'a RawPath,
    owner: Option<&'a Ownership>,
}

#[cfg(test)]
pub(crate) fn begin_at<S: Store>(
    store: &mut S,
    expected_sequence: u64,
    caller: &Caller,
    request: Request,
) -> Result<u64, S::Error> {
    let observed = store.load()?;
    reservation::require_none(&observed.state)?;
    begin_observed(store, &observed, expected_sequence, caller, request)
}

pub(crate) fn begin_reserved<S: Store>(store: &mut S, binding: Binding) -> Result<u64, S::Error> {
    let observed = store.load()?;
    let caller = reservation::matching(&observed.state, binding)?
        .caller()?
        .clone();
    let request = caller.request(observed.state.owner.as_ref())?;
    begin_observed(store, &observed, binding.sequence, &caller, request)
}

fn begin_observed<S: Store>(
    store: &mut S,
    observed: &Observed<S::Revision>,
    expected_sequence: u64,
    caller: &Caller,
    request: Request,
) -> Result<u64, S::Error> {
    let before = &observed.state;
    if before.sequence != expected_sequence {
        return Err(Fault::Conflict.into());
    }
    if before
        .journal
        .as_ref()
        .is_some_and(|record| matches!(record.phase, Phase::Prepared | Phase::Applied))
    {
        return Err(Fault::PendingTransaction.into());
    }
    let id = before
        .sequence
        .checked_add(1)
        .ok_or(Fault::SequenceExhausted)?;
    if caller.request(before.owner.as_ref())? != request {
        return Err(Fault::InvalidIdentity.into());
    }
    let source = Source {
        path: &before.path,
        owner: before.owner.as_ref(),
    };
    let (after_path, after_owner) = project(source, request, caller.package)?;
    let prepared = Snapshot {
        reservation: None,
        cancelled_binding: None,
        path: before.path.clone(),
        owner: before.owner.clone(),
        sequence: id,
        journal: Some(Journal {
            id,
            phase: Phase::Prepared,
            caller: caller.clone(),
            before_path: before.path.clone(),
            after_path,
            before_owner: before.owner.clone(),
            after_owner,
            rollback: None,
        }),
    };
    store.compare_replace(observed, prepared)?;
    Ok(id)
}

#[cfg(test)]
pub(crate) fn rollback_at<S: Store>(
    store: &mut S,
    expected_sequence: u64,
    caller: &Caller,
) -> Result<PathChange, S::Error> {
    let current = store.load()?;
    reservation::require_none(&current.state)?;
    if current.state.sequence == expected_sequence {
        return Ok(PathChange::Unchanged);
    }
    let wanted = expected_sequence
        .checked_add(1)
        .ok_or(Fault::SequenceExhausted)?;
    if current.state.sequence != wanted {
        return Err(Fault::Conflict.into());
    }
    rollback(store, wanted, caller)
}

fn project(
    before: Source<'_>,
    request: Request,
    package: u128,
) -> Result<(RawPath, Option<Ownership>), Fault> {
    match request {
        Request::Install { product, lease } => install(before, product, package, lease),
        Request::Repair { product } => repair(before, product),
        Request::Remove { product } => {
            let owner = owned(before, product)?;
            Ok((
                claim::remove(before.path, &owner.lease, &owner.claim)?,
                None,
            ))
        }
        Request::Transfer { from, to } => {
            let (path, owner) = repair(before, from)?;
            let mut owner = owner.ok_or(Fault::WrongOwner)?;
            owner.product = to;
            owner.package = package;
            Ok((path, Some(owner)))
        }
        Request::Upgrade { from, to, lease } => upgrade(before, from, to, package, lease),
    }
}

fn install(
    before: Source<'_>,
    product: u128,
    package: u128,
    lease: Lease,
) -> Result<(RawPath, Option<Ownership>), Fault> {
    if before.owner.is_some() {
        return Err(Fault::WrongOwner);
    }
    let acquired = claim::acquire(before.path, &lease)?;
    Ok((
        acquired.path,
        Some(Ownership {
            product,
            package,
            lease,
            claim: acquired.claim,
        }),
    ))
}

fn repair(before: Source<'_>, product: u128) -> Result<(RawPath, Option<Ownership>), Fault> {
    let mut owner = owned(before, product)?.clone();
    if let Some(parts) = before.path.split()?
        && path_edit::locate(parts.body, &owner.lease)?.is_some()
    {
        return Ok((before.path.clone(), Some(owner)));
    }
    let acquired = claim::acquire(before.path, &owner.lease)?;
    owner.claim = acquired.claim;
    Ok((acquired.path, Some(owner)))
}

fn upgrade(
    before: Source<'_>,
    from: u128,
    to: u128,
    package: u128,
    lease: Lease,
) -> Result<(RawPath, Option<Ownership>), Fault> {
    let owner = owned(before, from)?;
    let unowned = claim::remove(before.path, &owner.lease, &owner.claim)?;
    let acquired = claim::acquire(&unowned, &lease)?;
    Ok((
        acquired.path,
        Some(Ownership {
            product: to,
            package,
            lease,
            claim: acquired.claim,
        }),
    ))
}

fn owned(before: Source<'_>, product: u128) -> Result<&Ownership, Fault> {
    before
        .owner
        .filter(|owner| owner.product == product)
        .ok_or(Fault::WrongOwner)
}

pub(crate) fn validate_record(record: &Journal) -> Result<(), Fault> {
    let source = Source {
        path: &record.before_path,
        owner: record.before_owner.as_ref(),
    };
    let request = record.caller.request(source.owner)?;
    let (path, owner) = project(source, request, record.caller.package)?;
    if path == record.after_path && owner == record.after_owner {
        Ok(())
    } else {
        Err(Fault::CorruptState)
    }
}

#[cfg(test)]
pub(crate) fn preflight<S: Store>(
    store: &S,
    expected: u64,
    caller: &Caller,
) -> Result<(), S::Error> {
    let observed = store.load()?;
    let state = &observed.state;
    reservation::require_none(state)?;
    if state.sequence != expected {
        return Err(Fault::Conflict.into());
    }
    if state
        .journal
        .as_ref()
        .is_some_and(|entry| matches!(entry.phase, Phase::Prepared | Phase::Applied))
    {
        return Err(Fault::PendingTransaction.into());
    }
    let request = caller.request(state.owner.as_ref())?;
    project(
        Source {
            path: &state.path,
            owner: state.owner.as_ref(),
        },
        request,
        caller.package,
    )?;
    Ok(())
}

pub(crate) fn apply<S: Store>(
    store: &mut S,
    id: u64,
    caller: &Caller,
) -> Result<PathChange, S::Error> {
    let observed = store.load()?;
    let before = &observed.state;
    let record = journal(before, id, caller)?;
    match record.phase {
        Phase::Applied | Phase::Committed => return Ok(PathChange::Unchanged),
        Phase::RolledBack => return Err(Fault::WrongPhase.into()),
        Phase::Prepared => {}
    }
    if before.path != record.before_path || before.owner != record.before_owner {
        return Err(Fault::Conflict.into());
    }
    let mut after = Snapshot {
        reservation: None,
        cancelled_binding: None,
        path: record.after_path.clone(),
        owner: record.after_owner.clone(),
        journal: before.journal.clone(),
        sequence: before.sequence,
    };
    after.journal.as_mut().ok_or(Fault::WrongTransaction)?.phase = Phase::Applied;
    let changed = if before.path == after.path {
        PathChange::Unchanged
    } else {
        PathChange::Changed
    };
    store.compare_replace(&observed, after)?;
    Ok(changed)
}

pub(crate) fn commit<S: Store>(store: &mut S, id: u64, caller: &Caller) -> Result<(), S::Error> {
    let observed = store.load()?;
    let before = &observed.state;
    match journal(before, id, caller)?.phase {
        Phase::Committed => return Ok(()),
        Phase::Applied => {}
        Phase::Prepared | Phase::RolledBack => return Err(Fault::WrongPhase.into()),
    }
    let mut after = before.clone();
    after.journal.as_mut().ok_or(Fault::WrongTransaction)?.phase = Phase::Committed;
    store.compare_replace(&observed, after)
}

pub(crate) fn rollback<S: Store>(
    store: &mut S,
    id: u64,
    caller: &Caller,
) -> Result<PathChange, S::Error> {
    let observed = store.load()?;
    let before = &observed.state;
    let record = journal(before, id, caller)?;
    if record.phase == Phase::RolledBack {
        return Ok(PathChange::Unchanged);
    }
    let from = RollbackSource::try_from(record.phase)?;
    let expected_owner = match from {
        RollbackSource::Prepared => &record.before_owner,
        RollbackSource::Applied | RollbackSource::Committed => &record.after_owner,
    };
    if &before.owner != expected_owner {
        return Err(Fault::Conflict.into());
    }
    let receipt = RollbackReceipt {
        from,
        observed_path: before.path.clone(),
    };
    let projected = compensation::project(record, &receipt)?;
    let mut after = Snapshot {
        reservation: None,
        cancelled_binding: None,
        path: projected.path,
        owner: projected.owner,
        journal: before.journal.clone(),
        sequence: before.sequence,
    };
    let record = after.journal.as_mut().ok_or(Fault::WrongTransaction)?;
    record.phase = Phase::RolledBack;
    record.rollback = Some(receipt);
    let changed = if before.path == after.path {
        PathChange::Unchanged
    } else {
        PathChange::Changed
    };
    store.compare_replace(&observed, after)?;
    Ok(changed)
}

fn journal<'a>(snapshot: &'a Snapshot, id: u64, caller: &Caller) -> Result<&'a Journal, Fault> {
    reservation::require_none(snapshot)?;
    snapshot
        .journal
        .as_ref()
        .filter(|record| record.id == id && &record.caller == caller)
        .ok_or(Fault::WrongTransaction)
}
