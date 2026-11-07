// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::caller::{Caller, Goal};
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;
use crate::installer::state::Request;

use super::Memory;

const PACKAGE_BASE: u128 = 100;

pub(super) fn for_request(store: &Memory, request: &Request) -> Result<Caller, Fault> {
    let (product, predecessor, goal) = match request {
        Request::Install { product, lease } => (*product, None, Goal::Ensure(lease.clone())),
        Request::Repair { product } => (
            *product,
            None,
            Goal::Ensure(
                store
                    .state
                    .owner
                    .as_ref()
                    .ok_or(Fault::WrongOwner)?
                    .lease
                    .clone(),
            ),
        ),
        Request::Remove { product } => (*product, None, Goal::Remove),
        Request::Transfer { from, to } => (
            *to,
            (*from != *to).then_some(*from),
            Goal::Ensure(
                store
                    .state
                    .owner
                    .as_ref()
                    .ok_or(Fault::WrongOwner)?
                    .lease
                    .clone(),
            ),
        ),
        Request::Upgrade { from, to, lease } => (
            *to,
            (*from != *to).then_some(*from),
            Goal::Ensure(lease.clone()),
        ),
    };
    let package = store
        .state
        .owner
        .as_ref()
        .filter(|owner| owner.product == product)
        .map_or_else(
            || {
                product
                    .checked_add(PACKAGE_BASE)
                    .ok_or(Fault::InvalidIdentity)
            },
            |owner| Ok(owner.package),
        )?;
    Ok(Caller {
        product,
        package,
        nonce: InvocationNonce::from_bytes([1; 32])?,
        predecessor,
        goal,
    })
}
