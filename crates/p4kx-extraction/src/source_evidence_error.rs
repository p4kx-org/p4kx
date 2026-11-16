// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Mutex, MutexGuard};

use thiserror::Error;

use crate::source_registry::SourceRegistry;

/// A physical-source evidence registry cannot be used reliably.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SourceEvidenceError {
    /// A runtime use does not belong to this original evidence authority.
    #[error("source evidence belongs to another authority")]
    ForeignEvidence,
    /// A directory path range falls outside its original parsed Entry positions.
    #[error("directory path range exceeds its owning Archive")]
    InvalidDirectoryRange,
    /// An owner unwound while mutating the protected evidence.
    #[error("source evidence was interrupted during mutation")]
    PoisonedEvidence,
    /// The registry has finished and released its indexing state.
    #[error("source evidence registry is sealed")]
    SealedInventory,
}

pub(crate) fn lock(
    state: &Mutex<SourceRegistry>,
) -> Result<MutexGuard<'_, SourceRegistry>, SourceEvidenceError> {
    let registry = state
        .lock()
        .map_err(|_| SourceEvidenceError::PoisonedEvidence)?;
    if registry.is_sealed() {
        return Err(SourceEvidenceError::SealedInventory);
    }
    Ok(registry)
}
