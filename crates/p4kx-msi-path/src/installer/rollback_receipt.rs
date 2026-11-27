// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;
use crate::installer::raw_path::RawPath;
use crate::installer::state::Phase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RollbackSource {
    Prepared,
    Applied,
    Committed,
}

impl TryFrom<Phase> for RollbackSource {
    type Error = Fault;

    fn try_from(phase: Phase) -> Result<Self, Fault> {
        match phase {
            Phase::Prepared => Ok(Self::Prepared),
            Phase::Applied => Ok(Self::Applied),
            Phase::Committed => Ok(Self::Committed),
            Phase::RolledBack => Err(Fault::WrongPhase),
        }
    }
}

/// Exact input to the compensation that established the durable rollback owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RollbackReceipt {
    pub(crate) from: RollbackSource,
    pub(crate) observed_path: RawPath,
}
