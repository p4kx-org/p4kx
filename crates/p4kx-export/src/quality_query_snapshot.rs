// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Build, SourceInventory};

use super::{
    CatalogRecordCoverage, DatasetAvailability, QualityDistributionDefinition,
    QualityQuantizationDefinition,
};

/// Selected canonical definitions with the source inventory for their actual data values.
#[derive(Debug)]
#[must_use]
pub struct QualityDefinitionSnapshot<Row> {
    pub(super) build: Option<Build>,
    pub(super) availability: DatasetAvailability,
    pub(super) rows: Vec<Row>,
    pub(super) source_inventory: SourceInventory,
    pub(super) coverage: CatalogRecordCoverage,
}

/// Original distribution and location-override declarations of selected physical Records.
pub type DistributionSnapshot<'source> =
    QualityDefinitionSnapshot<QualityDistributionDefinition<'source>>;

/// Original quantization declarations without a resource inverse projection.
pub type QuantizationSnapshot<'source> =
    QualityDefinitionSnapshot<QualityQuantizationDefinition<'source>>;

impl<Row> QualityDefinitionSnapshot<Row> {
    /// Borrows the originating installed Build when identified.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Returns source declaration availability independently of selected row count.
    #[must_use]
    pub const fn availability(&self) -> DatasetAvailability {
        self.availability
    }

    /// Borrows rows in canonical GUID-text and physical Record order.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Borrows the physical sources and actual contributing uses of these rows.
    #[must_use]
    pub const fn source_inventory(&self) -> &SourceInventory {
        &self.source_inventory
    }

    /// Borrows selected header, body and field projection coverage.
    #[must_use]
    pub const fn coverage(&self) -> &CatalogRecordCoverage {
        &self.coverage
    }
}
