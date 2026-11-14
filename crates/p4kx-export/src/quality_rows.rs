// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;

use super::contract::{
    QualityDistributionDefinition, QualityQuantizationDefinition, QualityQuantizationResources,
};
use super::fallbacks::Fallbacks;
use crate::QuantizationSummary;
use crate::catalog_table::CatalogTable;

const QUANTIZATION_RECORD: &str = "CraftingQualityQuantizationRecord";

/// The retained canonical quality definitions and their genuine inverse relation.
#[derive(Default)]
pub(crate) struct QualityRows<'source> {
    pub(crate) distributions: Vec<QualityDistributionDefinition<'source>>,
    pub(crate) quantizations: Vec<QualityQuantizationDefinition<'source>>,
    pub(crate) inverse: Vec<QualityQuantizationResources>,
    pub(crate) fallbacks: Fallbacks,
    pub(crate) summary: QuantizationSummary,
    pub(crate) skipped_distributions: usize,
    pub(crate) skipped_quantizations: usize,
}

pub(crate) struct QualityTables<'source> {
    pub(crate) distributions: CatalogTable<QualityDistributionDefinition<'source>>,
    pub(crate) quantizations: CatalogTable<QualityQuantizationDefinition<'source>>,
    pub(crate) resources: CatalogTable<QualityQuantizationResources>,
}

impl<'source> QualityRows<'source> {
    pub(crate) fn take_tables(&mut self, forge: &DataForge<'_>) -> QualityTables<'source> {
        QualityTables {
            distributions: CatalogTable::new(
                forge,
                &[
                    "CraftingQualityDistributionRecord",
                    "CraftingQualityLocationOverrideRecord",
                ],
                std::mem::take(&mut self.distributions),
            ),
            quantizations: CatalogTable::new(
                forge,
                &[QUANTIZATION_RECORD],
                std::mem::take(&mut self.quantizations),
            ),
            resources: CatalogTable::new(
                forge,
                &[QUANTIZATION_RECORD],
                std::mem::take(&mut self.inverse),
            ),
        }
    }
}
