// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogRefusal, ExportHealth};
use p4kx_extraction::SourceInventory;

/// The paired immutable report and inventory returned by the actual export refusal.
#[derive(Debug)]
pub(super) struct Refusal {
    report: CatalogRefusal,
}

impl Refusal {
    pub(super) const fn new(report: CatalogRefusal) -> Self {
        Self { report }
    }

    pub(super) fn health(&self) -> &ExportHealth {
        self.report.health()
    }

    pub(super) fn inventory(&self) -> &SourceInventory {
        self.report.source_inventory()
    }
}
