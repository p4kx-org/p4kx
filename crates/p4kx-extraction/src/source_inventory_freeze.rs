// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::sync::Arc;

use thiserror::Error;

use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::source_inventory::{PackedUseRows, SourceInventory};

use super::SourceUses;

#[cfg(test)]
#[path = "source_inventory_batch_compaction_tests.rs"]
mod batch_compaction_tests;
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
    /// Counts the distinct uses still held in the capture registry.
    ///
    /// # Errors
    /// Returns an error when the evidence registry is unavailable.
    pub fn registered_use_count(&self) -> Result<usize, SourceEvidenceError> {
        Ok(source_evidence_error::lock(&self.registry)?.uses.len())
    }

    /// Counts registered uses stored as compact DataForge coordinates.
    ///
    /// # Errors
    /// Returns an error when the evidence registry is unavailable.
    pub fn packed_use_count(&self) -> Result<usize, SourceEvidenceError> {
        Ok(source_evidence_error::lock(&self.registry)?
            .uses
            .packed_len())
    }

    /// Counts packed key rows and UTF-8 bytes by site and ownership.
    /// Buckets are direct registry-only, direct shared, typed registry-only and typed shared.
    /// The checkpoint runs under the registry lock and must not re-enter it.
    ///
    /// # Errors
    /// Returns a checkpoint error or an error when the registry is unavailable.
    #[doc(hidden)]
    pub fn packed_key_storage_census<E: From<SourceEvidenceError>>(
        &self,
        mut check: impl FnMut() -> Result<(), E>,
    ) -> Result<[(usize, usize); 4], E> {
        let registry = source_evidence_error::lock(&self.registry)?;
        let mut buckets = [(0, 0); 4];
        let [direct_only, direct_shared, typed_only, typed_shared] = &mut buckets;
        for (key, row) in registry.uses.packed_values() {
            check()?;
            let (rows, bytes) = match (row.has_direct_field(), Arc::strong_count(key) == 1) {
                (true, true) => &mut *direct_only,
                (true, false) => &mut *direct_shared,
                (false, true) => &mut *typed_only,
                (false, false) => &mut *typed_shared,
            };
            *rows += 1;
            *bytes += key.len();
        }
        drop(registry);
        Ok(buckets)
    }

    /// Packs unheld DataForge coordinate uses and rechecks uses deferred by batch compaction.
    /// The checkpoint runs under the registry lock and must not re-enter it.
    ///
    /// # Errors
    /// Returns the checkpoint error or an error when the registry is sealed or interrupted.
    pub fn compact_record_uses<E: From<SourceEvidenceError>>(
        &self,
        check: impl FnMut() -> Result<(), E>,
    ) -> Result<(), E> {
        source_evidence_error::lock(&self.registry)?
            .uses
            .compact_pending(check)
    }

    /// Packs unheld DataForge uses, deferring uses held by the active projection.
    /// Call between batches of one projection. The checkpoint runs under the registry lock
    /// and must not re-enter it. A later `compact_record_uses` rechecks deferred uses.
    ///
    /// # Errors
    /// Returns the checkpoint error or an error when the registry is sealed or interrupted.
    pub fn compact_record_use_batch<E: From<SourceEvidenceError>>(
        &self,
        check: impl FnMut() -> Result<(), E>,
    ) -> Result<(), E> {
        source_evidence_error::lock(&self.registry)?
            .uses
            .compact_batch(check)
    }

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
        if self.provenance_mode() == crate::ProvenanceMode::MarkersOnly {
            return Ok(SourceInventory::markers_only());
        }
        let sources = collect_rows(registry.sources.values().cloned(), &mut check)?;
        let uses = collect_rows(registry.uses.values().cloned(), &mut check)?;
        let packed = registry
            .uses
            .packed_values()
            .map(|(id, row)| (Arc::clone(id), row.clone()));
        let packed = collect_rows(packed, &mut check)?;
        drop(registry);
        Ok(SourceInventory::new(
            sources,
            uses,
            PackedUseRows::Snapshot(packed),
        ))
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
        let mut uses = std::mem::take(&mut registry.uses);
        drop(registry);
        if self.provenance_mode() == crate::ProvenanceMode::MarkersOnly {
            check()?;
            return Ok(SourceInventory::markers_only());
        }
        uses.compact_pending(&mut check)?;
        let sources = collect_rows(sources.into_values(), &mut check)?;
        let (uses, packed) = uses.into_parts();
        let uses = collect_rows(uses, &mut check)?;
        for _ in packed.values() {
            check()?;
        }
        Ok(SourceInventory::new(
            sources,
            uses,
            PackedUseRows::Sealed(packed),
        ))
    }
}

fn collect_rows<Row, E>(
    rows: impl ExactSizeIterator<Item = Row>,
    check: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<Row>, E> {
    let mut snapshot = Vec::with_capacity(rows.len());
    for row in rows {
        check()?;
        snapshot.push(row);
    }
    Ok(snapshot)
}
