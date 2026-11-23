// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceInventory, SourceUses};

use crate::options::CatalogRun;
use crate::{CatalogError, CatalogProvenance, ExportHealth};

use super::parts;

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

/// The owned evidence of a refusal before sealing its source inventory.
pub(super) struct PendingRefusal {
    health: Box<ExportHealth>,
    uses: SourceUses,
}

impl PendingRefusal {
    pub(super) fn new(health: ExportHealth, uses: SourceUses) -> Self {
        Self {
            health: Box::new(health),
            uses,
        }
    }

    pub(super) fn freeze(self, run: CatalogRun<'_, '_>) -> Result<CatalogRefusal, CatalogError> {
        Ok(CatalogRefusal {
            health: self.health,
            provenance: parts::freeze_provenance(&self.uses, &[], &[], run)?,
        })
    }
}
