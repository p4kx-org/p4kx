// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use winreg::enums;

use crate::installer::caller::{Caller, Goal};
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;
use crate::installer::journal_codec;
use crate::installer::raw_path::RawPath;
use crate::installer::registry_adapter;
use crate::installer::registry_store::{RawValue, RegistrySnapshot};
use crate::installer::reservation::Reservation;
use crate::installer::sequence;
use crate::installer::tests;

fn image() -> Result<RegistrySnapshot, Fault> {
    let mut state = tests::installed(RawPath::Absent)?;
    let owner = state.owner.as_ref().ok_or(Fault::MissingOwnership)?;
    let caller = Caller {
        product: owner.product,
        package: owner.package,
        predecessor: None,
        nonce: InvocationNonce::from_bytes([9; 32])?,
        goal: Goal::Ensure(owner.lease.clone()),
    };
    let reservation = Reservation {
        base_sequence: state.sequence,
        caller,
    };
    let binding = reservation.binding();
    state.reservation = Some(reservation);
    Ok(RegistrySnapshot {
        path: None,
        state: Some(journal_codec::encode(&state)?),
        sequence: Some(RawValue {
            kind: enums::REG_SZ,
            bytes: sequence::encode(state.sequence),
        }),
        binding: Some(RawValue {
            kind: enums::REG_SZ,
            bytes: binding.registry_bytes(),
        }),
    })
}
#[test]
fn raw_image_requires_the_exact_same_binding_as_its_fully_decoded_state() -> Result<(), Fault> {
    let original = image()?;
    let state = registry_adapter::stored(&original)?;
    assert!(
        state.reservation.is_some(),
        "matching scalar and binding yield a complete decoded reservation"
    );
    let mut changed = original.clone();
    changed.binding = None;
    assert_eq!(
        registry_adapter::stored(&changed),
        Err(Fault::CorruptState),
        "missing transport cannot be inferred from the state"
    );
    let mut changed = original.clone();
    changed.binding.as_mut().ok_or(Fault::InvalidBinding)?.kind = enums::REG_EXPAND_SZ;
    assert_eq!(
        registry_adapter::stored(&changed),
        Err(Fault::CorruptState),
        "AppSearch transport must be raw REG_SZ"
    );
    let mut changed = original;
    changed.sequence = Some(RawValue {
        kind: enums::REG_SZ,
        bytes: sequence::encode(2),
    });
    assert_eq!(
        registry_adapter::stored(&changed),
        Err(Fault::CorruptState),
        "torn scalar and WAL are not accepted"
    );
    Ok(())
}

#[test]
fn orphan_and_foreign_registry_bindings_are_not_valid_empty_states() -> Result<(), Fault> {
    let mut original = image()?;
    original.state = None;
    original.sequence = None;
    assert_eq!(
        registry_adapter::stored(&original),
        Err(Fault::CorruptState),
        "standalone Binding is not a valid initial state"
    );
    let state = tests::installed(RawPath::Absent)?;
    original.state = Some(journal_codec::encode(&state)?);
    original.sequence = Some(RawValue {
        kind: enums::REG_SZ,
        bytes: sequence::encode(state.sequence),
    });
    assert_eq!(
        registry_adapter::stored(&original),
        Err(Fault::CorruptState),
        "terminal WAL does not authorise stale AppSearch transport"
    );
    Ok(())
}
