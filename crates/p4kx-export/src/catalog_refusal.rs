// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceInventory;

use crate::{CatalogProvenance, ExportHealth};

/// One normal-publication refusal and its inseparable immutable source inventory.
///
/// ~~~compile_fail
/// use p4kx_export::CatalogRefusal;
/// fn replace_inventory(mut own: CatalogRefusal, foreign: CatalogRefusal) {
///     own.provenance = foreign.provenance;
/// }
/// ~~~
#[derive(Debug)]
pub struct CatalogRefusal {
    health: Box<ExportHealth>,
    provenance: CatalogProvenance,
}

impl CatalogRefusal {
    pub(super) const fn new(health: Box<ExportHealth>, provenance: CatalogProvenance) -> Self {
        Self { health, provenance }
    }

    /// Borrows the source and projection health that prevented publication.
    #[must_use]
    pub fn health(&self) -> &ExportHealth {
        &self.health
    }

    /// Borrows this exact refused report's immutable inventory.
    #[must_use]
    pub fn source_inventory(&self) -> &SourceInventory {
        self.provenance.inventory()
    }
}
