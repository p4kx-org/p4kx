// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Identity {
    pub(crate) product: u128,
    pub(crate) package: u128,
    pub(crate) nonce: InvocationNonce,
}

impl Identity {
    pub(crate) const fn validate(self) -> Result<(), Fault> {
        if self.product == 0 || self.package == 0 {
            return Err(Fault::InvalidIdentity);
        }
        Ok(())
    }
}
