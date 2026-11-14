// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;

use thiserror::Error;

use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::source_inventory::SourceInventory;

use super::SourceUses;

/// Failure to freeze the physical sources and uses of a projection.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SourceInventoryFreezeError<E> {
    /// The source evidence registry could not be used.
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
    /// The caller interrupted inventory collection.
    #[error("inventory collection was interrupted")]
    Interrupted(E),
}

impl SourceUses {
    /// Freezes sorted source/use rows after projection for repeatable serialization passes.
    ///
    /// # Errors
    /// Returns an error when a mutation interrupted the shared evidence registry.
    pub fn freeze(&self) -> Result<SourceInventory, SourceEvidenceError> {
        match self.freeze_with(|| Ok::<(), Infallible>(())) {
            Ok(inventory) => Ok(inventory),
            Err(SourceInventoryFreezeError::Evidence(error)) => Err(error),
            Err(SourceInventoryFreezeError::Interrupted(never)) => match never {},
        }
    }

    /// Freezes the inventory and checks before cloning each physical source or use row.
    /// The checkpoint runs under the inventory lock and must not re-enter this inventory.
    ///
    /// # Errors
    /// Returns interrupted evidence or the caller's checkpoint error.
    pub fn freeze_with<E>(
        &self,
        mut check: impl FnMut() -> Result<(), E>,
    ) -> Result<SourceInventory, SourceInventoryFreezeError<E>> {
        check().map_err(SourceInventoryFreezeError::Interrupted)?;
        let registry = source_evidence_error::lock(&self.registry)?;
        let sources = registry
            .sources
            .values()
            .map(|source| {
                check().map_err(SourceInventoryFreezeError::Interrupted)?;
                Ok(source.clone())
            })
            .collect::<Result<_, SourceInventoryFreezeError<E>>>()?;
        let uses = registry
            .uses
            .values()
            .map(|description| {
                check().map_err(SourceInventoryFreezeError::Interrupted)?;
                Ok(description.as_ref().clone())
            })
            .collect::<Result<_, SourceInventoryFreezeError<E>>>()?;
        drop(registry);
        Ok(SourceInventory { sources, uses })
    }
}
