// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::sync::Arc;

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

    /// Freezes the inventory and checks before retaining each physical source or use row.
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
        let sources = retain_rows(registry.sources.values(), &mut check)?;
        let uses = retain_rows(registry.uses.values(), &mut check)?;
        drop(registry);
        Ok(SourceInventory::new(sources, uses))
    }
}

fn retain_rows<'row, Row: 'row, E>(
    rows: impl ExactSizeIterator<Item = &'row Arc<Row>>,
    check: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<Arc<Row>>, SourceInventoryFreezeError<E>> {
    let mut snapshot = Vec::with_capacity(rows.len());
    for row in rows {
        check().map_err(SourceInventoryFreezeError::Interrupted)?;
        snapshot.push(Arc::clone(row));
    }
    Ok(snapshot)
}
