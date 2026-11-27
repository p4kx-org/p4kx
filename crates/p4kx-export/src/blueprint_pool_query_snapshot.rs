// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Build, SourceInventory};

use super::{BlueprintPoolDefinition, BlueprintPoolWeights};
use crate::catalog_queries::{CatalogRecordCoverage, DatasetAvailability};

/// Original physical Blueprint-pool rows with their own source and coverage context.
#[derive(Debug)]
#[must_use]
pub struct BlueprintPoolSnapshot<'source> {
    pub(super) build: Option<Build>,
    pub(super) availability: DatasetAvailability,
    pub(super) rows: Vec<BlueprintPoolDefinition<'source>>,
    pub(super) weights: Vec<BlueprintPoolWeights>,
    pub(super) source_inventory: SourceInventory,
    pub(super) coverage: CatalogRecordCoverage,
}

impl<'source> BlueprintPoolSnapshot<'source> {
    /// Borrows the installed Build when one was identified.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Returns availability of the declared original pool family.
    #[must_use]
    pub const fn availability(&self) -> DatasetAvailability {
        self.availability
    }

    /// Borrows canonical GUID-ordered rows with physical-index ties.
    #[must_use]
    pub fn rows(&self) -> &[BlueprintPoolDefinition<'source>] {
        &self.rows
    }

    /// Borrows weight projections in the same physical definition order.
    #[must_use]
    pub fn weights(&self) -> &[BlueprintPoolWeights] {
        &self.weights
    }

    /// Borrows the original source/use inventory of these returned cells.
    #[must_use]
    pub const fn source_inventory(&self) -> &SourceInventory {
        &self.source_inventory
    }

    /// Borrows selected original header, body and field coverage.
    #[must_use]
    pub const fn coverage(&self) -> &CatalogRecordCoverage {
        &self.coverage
    }
}
