// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::binding::Binding;
use crate::installer::caller::Caller;
use crate::installer::claim::Claim;
use crate::installer::fault::Fault;
use crate::installer::lease::Lease;
use crate::installer::raw_path::RawPath;
use crate::installer::reservation::Reservation;
use crate::installer::rollback_receipt::RollbackReceipt;
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ownership {
    pub(crate) product: u128,
    pub(crate) package: u128,
    pub(crate) lease: Lease,
    pub(crate) claim: Claim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Prepared,
    Applied,
    Committed,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Journal {
    pub(crate) id: u64,
    pub(crate) phase: Phase,
    pub(crate) caller: Caller,
    pub(crate) before_path: RawPath,
    pub(crate) after_path: RawPath,
    pub(crate) before_owner: Option<Ownership>,
    pub(crate) after_owner: Option<Ownership>,
    pub(crate) rollback: Option<RollbackReceipt>,
}

/// A committed journal remains available for later MSI commit-action rollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub(crate) reservation: Option<Reservation>,
    pub(crate) cancelled_binding: Option<Binding>,
    pub(crate) path: RawPath,
    pub(crate) owner: Option<Ownership>,
    pub(crate) journal: Option<Journal>,
    pub(crate) sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathChange {
    Unchanged,
    Changed,
}

/// The adapter atomically compares and replaces PATH, ownership and journal.
pub(crate) trait Store {
    type Revision;
    type Error: From<Fault>;
    fn load(&self) -> Result<Observed<Self::Revision>, Self::Error>;
    fn compare_replace(
        &mut self,
        expected: &Observed<Self::Revision>,
        replacement: Snapshot,
    ) -> Result<(), Self::Error>;
}

pub(crate) struct Observed<R> {
    pub(crate) state: Snapshot,
    pub(crate) revision: R,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Request {
    Install { product: u128, lease: Lease },
    Repair { product: u128 },
    Remove { product: u128 },
    Transfer { from: u128, to: u128 },
    Upgrade { from: u128, to: u128, lease: Lease },
}
