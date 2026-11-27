// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Build, SourceInventory};

use super::BlueprintRewardSource;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability};

/// Selects original owners or physical targets of the canonical reward-source survey.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum BlueprintRewardSelection {
    /// Reads selected source owners and their demanded pool dependencies.
    SourceOwners(CatalogRecordSelection),
    /// Surveys all source Records for links to the selected physical pool targets.
    TargetPools(CatalogRecordSelection),
}

/// Original reward occurrences with separate source-survey and pool-dependency coverage.
#[derive(Debug)]
#[must_use]
pub struct BlueprintRewardSnapshot<'source> {
    pub(super) build: Option<Build>,
    pub(super) availability: DatasetAvailability,
    pub(super) selection: BlueprintRewardSelection,
    pub(super) rows: Vec<BlueprintRewardSource<'source>>,
    pub(super) source_inventory: SourceInventory,
    pub(super) source_coverage: CatalogRecordCoverage,
    pub(super) pool_coverage: CatalogRecordCoverage,
    pub(super) source_records_read: usize,
    pub(super) pool_records_read: usize,
}

impl<'source> BlueprintRewardSnapshot<'source> {
    /// Borrows the installed Build when one was identified.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Returns the original pool-family availability, independently of selected row count.
    #[must_use]
    pub const fn availability(&self) -> DatasetAvailability {
        self.availability
    }

    /// Returns the selection and whether its source survey is scoped or global.
    #[must_use]
    pub const fn selection(&self) -> BlueprintRewardSelection {
        self.selection
    }

    /// Borrows rows in physical source Record order and original occurrence order.
    #[must_use]
    pub fn rows(&self) -> &[BlueprintRewardSource<'source>] {
        &self.rows
    }

    /// Borrows original sources and uses, including actual admission and dependency evidence.
    #[must_use]
    pub const fn source_inventory(&self) -> &SourceInventory {
        &self.source_inventory
    }

    /// Borrows gaps in the actual selected-owner or global source survey.
    /// An empty target selection has zero source reads and performs no source survey.
    #[must_use]
    pub const fn source_coverage(&self) -> &CatalogRecordCoverage {
        &self.source_coverage
    }

    /// Borrows gaps in classification and reading of demanded physical pool dependencies.
    #[must_use]
    pub const fn pool_coverage(&self) -> &CatalogRecordCoverage {
        &self.pool_coverage
    }

    /// Returns the number of actual source-body attempts, including zero-match and failed reads.
    #[must_use]
    pub const fn source_records_read(&self) -> usize {
        self.source_records_read
    }

    /// Returns the number of distinct physical pool-body attempts.
    #[must_use]
    pub const fn pool_records_read(&self) -> usize {
        self.pool_records_read
    }
}
