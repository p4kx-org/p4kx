// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::caller::{self, Caller, Goal};
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Declaration {
    pub(crate) product: u128,
    pub(crate) package: u128,
    pub(crate) predecessor: Option<u128>,
    pub(crate) goal: Goal,
}

impl Declaration {
    pub(crate) fn validate(&self) -> Result<(), Fault> {
        caller::validate_identity(self.product, self.package, self.predecessor, &self.goal)
    }

    pub(crate) fn bind(self, nonce: InvocationNonce) -> Result<Caller, Fault> {
        self.validate()?;
        Ok(Caller {
            product: self.product,
            package: self.package,
            predecessor: self.predecessor,
            goal: self.goal,
            nonce,
        })
    }
}
