// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::binding::Binding;
use crate::installer::caller::Caller;
use crate::installer::declaration::Declaration;
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;
use crate::installer::reservation_identity::Identity;
use crate::installer::state::{Phase, Snapshot, Store};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reservation {
    pub(crate) base_sequence: u64,
    pub(crate) plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Plan {
    Neutral(Identity),
    Sealed(Caller),
}

impl Reservation {
    #[must_use]
    pub(crate) const fn identity(&self) -> Identity {
        match &self.plan {
            Plan::Neutral(identity) => *identity,
            Plan::Sealed(caller) => Identity {
                product: caller.product,
                package: caller.package,
                nonce: caller.nonce,
            },
        }
    }

    #[must_use]
    pub(crate) const fn binding(&self) -> Binding {
        let identity = self.identity();
        Binding {
            sequence: self.base_sequence,
            product: identity.product,
            package: identity.package,
            nonce: identity.nonce,
        }
    }

    pub(crate) const fn caller(&self) -> Result<&Caller, Fault> {
        match &self.plan {
            Plan::Neutral(_) => Err(Fault::WrongPhase),
            Plan::Sealed(caller) => Ok(caller),
        }
    }

    pub(crate) const fn set_nonce(&mut self, nonce: InvocationNonce) {
        match &mut self.plan {
            Plan::Neutral(identity) => identity.nonce = nonce,
            Plan::Sealed(caller) => caller.nonce = nonce,
        }
    }
}

pub(crate) fn create<S: Store>(store: &mut S, identity: Identity) -> Result<Binding, S::Error> {
    let observed = store.load()?;
    let before = &observed.state;
    validate(before)?;
    if before.reservation.is_some() {
        return Err(Fault::PendingReservation.into());
    }
    terminal(before)?;
    identity.validate()?;
    fresh(before, identity.nonce)?;
    before
        .sequence
        .checked_add(1)
        .ok_or(Fault::SequenceExhausted)?;
    let reservation = Reservation {
        base_sequence: before.sequence,
        plan: Plan::Neutral(identity),
    };
    let binding = reservation.binding();
    let mut after = before.clone();
    after.reservation = Some(reservation);
    after.cancelled_binding = None;
    store.compare_replace(&observed, after)?;
    Ok(binding)
}

pub(crate) fn seal<S: Store>(
    store: &mut S,
    expected: Binding,
    declaration: Declaration,
) -> Result<(), S::Error> {
    let observed = store.load()?;
    let original = matching(&observed.state, expected)?;
    if !matches!(original.plan, Plan::Neutral(_)) {
        return Err(Fault::WrongPhase.into());
    }
    if declaration.product != expected.product || declaration.package != expected.package {
        return Err(Fault::InvalidIdentity.into());
    }
    let caller = declaration.bind(expected.nonce)?;
    caller.request(observed.state.owner.as_ref())?;
    let mut replacement = original.clone();
    replacement.plan = Plan::Sealed(caller);
    let mut after = observed.state.clone();
    after.reservation = Some(replacement);
    store.compare_replace(&observed, after)
}

pub(crate) fn recover<S: Store>(
    store: &mut S,
    expected: Binding,
    nonce: InvocationNonce,
) -> Result<Binding, S::Error> {
    let observed = store.load()?;
    let original = matching(&observed.state, expected)?;
    if nonce == original.identity().nonce {
        return Err(Fault::InvalidNonce.into());
    }
    fresh(&observed.state, nonce)?;
    let mut replacement = original.clone();
    replacement.set_nonce(nonce);
    let binding = replacement.binding();
    let mut after = observed.state.clone();
    after.reservation = Some(replacement);
    store.compare_replace(&observed, after)?;
    Ok(binding)
}

pub(crate) fn cancel<S: Store>(store: &mut S, expected: Binding) -> Result<(), S::Error> {
    let observed = store.load()?;
    matching(&observed.state, expected)?;
    let mut after = observed.state.clone();
    after.reservation = None;
    after.cancelled_binding = Some(expected);
    store.compare_replace(&observed, after)
}

pub(crate) fn matching(snapshot: &Snapshot, binding: Binding) -> Result<&Reservation, Fault> {
    let reservation = snapshot
        .reservation
        .as_ref()
        .ok_or(Fault::MissingReservation)?;
    if reservation.binding() != binding {
        return Err(Fault::WrongTransaction);
    }
    validate(snapshot)?;
    Ok(reservation)
}

pub(crate) fn validate(snapshot: &Snapshot) -> Result<(), Fault> {
    if snapshot.reservation.is_some() && snapshot.cancelled_binding.is_some() {
        return Err(Fault::CorruptState);
    }
    if let Some(binding) = snapshot.cancelled_binding {
        binding.validate()?;
        if binding.sequence != snapshot.sequence {
            return Err(Fault::CorruptState);
        }
        binding
            .sequence
            .checked_add(1)
            .ok_or(Fault::SequenceExhausted)?;
        terminal(snapshot)?;
        fresh_terminal(snapshot, binding.nonce)?;
    }
    let Some(reservation) = &snapshot.reservation else {
        return Ok(());
    };
    if reservation.base_sequence != snapshot.sequence {
        return Err(Fault::CorruptState);
    }
    terminal(snapshot)?;
    reservation.identity().validate()?;
    if let Plan::Sealed(caller) = &reservation.plan {
        caller.request(snapshot.owner.as_ref())?;
    }
    fresh(snapshot, reservation.identity().nonce)?;
    snapshot
        .sequence
        .checked_add(1)
        .ok_or(Fault::SequenceExhausted)?;
    Ok(())
}

pub(crate) const fn require_none(snapshot: &Snapshot) -> Result<(), Fault> {
    if snapshot.reservation.is_some() || snapshot.cancelled_binding.is_some() {
        return Err(Fault::PendingReservation);
    }
    Ok(())
}

fn fresh(snapshot: &Snapshot, nonce: InvocationNonce) -> Result<(), Fault> {
    if snapshot
        .cancelled_binding
        .is_some_and(|binding| binding.nonce == nonce)
    {
        return Err(Fault::InvalidNonce);
    }
    fresh_terminal(snapshot, nonce)
}

fn fresh_terminal(snapshot: &Snapshot, nonce: InvocationNonce) -> Result<(), Fault> {
    if snapshot
        .journal
        .as_ref()
        .is_some_and(|record| record.caller.nonce == nonce)
    {
        return Err(Fault::InvalidNonce);
    }
    Ok(())
}

fn terminal(snapshot: &Snapshot) -> Result<(), Fault> {
    if snapshot
        .journal
        .as_ref()
        .is_some_and(|record| matches!(record.phase, Phase::Prepared | Phase::Applied))
    {
        return Err(Fault::PendingTransaction);
    }
    Ok(())
}
