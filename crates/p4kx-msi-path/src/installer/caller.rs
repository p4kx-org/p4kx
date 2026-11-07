// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;
use crate::installer::lease::Lease;
use crate::installer::state::{Ownership, Request};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Goal {
    Ensure(Lease),
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Caller {
    pub(crate) product: u128,
    pub(crate) package: u128,
    pub(crate) nonce: InvocationNonce,
    pub(crate) predecessor: Option<u128>,
    pub(crate) goal: Goal,
}

impl Caller {
    pub(crate) fn validate(&self) -> Result<(), Fault> {
        validate_identity(self.product, self.package, self.predecessor, &self.goal)
    }
    pub(crate) fn request(&self, owner: Option<&Ownership>) -> Result<Request, Fault> {
        self.validate()?;
        let Goal::Ensure(lease) = &self.goal else {
            let owner = owner.ok_or(Fault::MissingOwnership)?;
            self.current_owner(owner.product, owner.package)?;
            return Ok(Request::Remove {
                product: self.product,
            });
        };
        let Some(owner) = owner else {
            return Ok(Request::Install {
                product: self.product,
                lease: lease.clone(),
            });
        };
        if owner.product == self.product {
            self.current_owner(owner.product, owner.package)?;
            if owner.lease == *lease {
                return Ok(Request::Repair {
                    product: self.product,
                });
            }
        } else if self.predecessor != Some(owner.product) {
            return Err(Fault::WrongOwner);
        }
        if owner.lease == *lease {
            Ok(Request::Transfer {
                from: owner.product,
                to: self.product,
            })
        } else {
            Ok(Request::Upgrade {
                from: owner.product,
                to: self.product,
                lease: lease.clone(),
            })
        }
    }

    const fn current_owner(&self, product: u128, package: u128) -> Result<(), Fault> {
        if product != self.product || package != self.package || self.predecessor.is_some() {
            return Err(Fault::WrongOwner);
        }
        Ok(())
    }
}

pub(crate) fn validate_identity(
    product: u128,
    package: u128,
    predecessor: Option<u128>,
    goal: &Goal,
) -> Result<(), Fault> {
    if product == 0
        || package == 0
        || predecessor == Some(0)
        || predecessor == Some(product)
        || (matches!(goal, Goal::Remove) && predecessor.is_some())
    {
        return Err(Fault::InvalidIdentity);
    }
    Ok(())
}
