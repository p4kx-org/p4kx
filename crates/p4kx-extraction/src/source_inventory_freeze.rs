// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::sync::Arc;

use thiserror::Error;

use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::source_inventory::SourceInventory;

use super::SourceUses;

#[cfg(test)]
#[path = "source_inventory_finish_tests.rs"]
mod finish_tests;

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
    /// Returns an error when the shared evidence registry is sealed or interrupted.
    pub fn freeze(&self) -> Result<SourceInventory, SourceEvidenceError> {
        match self.freeze_with(|| Ok::<(), Infallible>(())) {
            Ok(inventory) => Ok(inventory),
            Err(SourceInventoryFreezeError::Evidence(error)) => Err(error),
            Err(SourceInventoryFreezeError::Interrupted(never)) => match never {},
        }
    }

    /// Freezes the inventory and checks before retaining each physical source or use row.
    /// The checkpoint runs under the inventory lock and must not re-enter this inventory.
    ///
    /// # Errors
    /// Returns sealed or interrupted evidence or the caller's checkpoint error.
    pub fn freeze_with<E>(
        &self,
        mut check: impl FnMut() -> Result<(), E>,
    ) -> Result<SourceInventory, SourceInventoryFreezeError<E>> {
        let mut check = || check().map_err(SourceInventoryFreezeError::Interrupted);
        check()?;
        let registry = source_evidence_error::lock(&self.registry)?;
        let sources = collect_rows(registry.sources.values().cloned(), &mut check)?;
        let uses = collect_rows(registry.uses.values().cloned(), &mut check)?;
        drop(registry);
        Ok(SourceInventory::new(sources, uses))
    }

    /// Seals the registry and moves its sorted source and use rows into an immutable inventory.
    /// Previously captured uses can still bind field origins through this authority.
    ///
    /// # Errors
    /// Returns errors from the checkpoint or registry. A failed initial checkpoint leaves the
    /// registry open. Once sealed, later checkpoint errors leave it sealed across all handles.
    pub fn finish_with<E: From<SourceEvidenceError>>(
        &self,
        mut check: impl FnMut() -> Result<(), E>,
    ) -> Result<SourceInventory, E> {
        check()?;
        let mut registry = source_evidence_error::lock(&self.registry)?;
        registry.seal();
        let sources = std::mem::take(&mut registry.sources);
        let uses = std::mem::take(&mut registry.uses);
        drop(registry);
        let sources = collect_rows(sources.into_values(), &mut check)?;
        let uses = collect_rows(uses.into_values(), &mut check)?;
        Ok(SourceInventory::new(sources, uses))
    }
}

fn collect_rows<Row, E>(
    rows: impl ExactSizeIterator<Item = Arc<Row>>,
    check: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<Arc<Row>>, E> {
    let mut snapshot = Vec::with_capacity(rows.len());
    for row in rows {
        check()?;
        snapshot.push(row);
    }
    Ok(snapshot)
}
