// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Mutex, MutexGuard};

use thiserror::Error;

/// A physical-source evidence registry cannot be used reliably.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SourceEvidenceError {
    /// An owner unwound while mutating the protected evidence.
    #[error("source evidence was interrupted during mutation")]
    PoisonedEvidence,
}

pub(crate) fn lock<T>(state: &Mutex<T>) -> Result<MutexGuard<'_, T>, SourceEvidenceError> {
    state
        .lock()
        .map_err(|_| SourceEvidenceError::PoisonedEvidence)
}
