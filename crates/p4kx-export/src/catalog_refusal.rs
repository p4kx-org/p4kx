// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use p4kx_extraction::{SourceInventory, SourceUses};

use crate::options::CatalogRun;
use crate::{CatalogError, CatalogProvenance, ExportDiagnosticCode, ExportHealth, Kind};

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

impl fmt::Display for CatalogRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "catalog export is incomplete; explicit diagnostic partial export is required",
        )?;
        for diagnostic in self.health.diagnostics() {
            if diagnostic.kind != Some(Kind::ShopInventories) {
                continue;
            }
            let reason = match diagnostic.code {
                ExportDiagnosticCode::ArchiveKeyRequired => "an Archive key is required",
                ExportDiagnosticCode::WrongArchiveKey => "the Archive key was rejected",
                _ => continue,
            };
            write!(
                formatter,
                ". Required reads in definitions.shopInventories blocked because {reason} ({})",
                diagnostic.count,
            )?;
        }
        Ok(())
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
