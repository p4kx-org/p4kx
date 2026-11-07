// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A state transition that cannot preserve exact ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    InvalidRawPath,
    InvalidLease,
    ExistingLease,
    AmbiguousLease,
    WrongOwner,
    PendingTransaction,
    WrongTransaction,
    WrongPhase,
    Conflict,
    SequenceExhausted,
    CorruptState,
    StateLimit,
    InvalidIdentity,
    MissingOwnership,
    InvalidArguments,
    InvalidFolder,
    ArgumentLimit,
    InvalidNonce,
    InvalidBinding,
    MissingReservation,
    PendingReservation,
}

impl std::fmt::Display for Fault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "installer PATH transition: {self:?}")
    }
}

impl std::error::Error for Fault {}
