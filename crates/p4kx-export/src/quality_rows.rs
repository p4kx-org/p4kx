// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::contract::{
    QualityDistributionDefinition, QualityQuantizationDefinition, QualityQuantizationResources,
};
use crate::QuantizationSummary;
use crate::guid_text::CatalogGuid;

/// The retained canonical quality definitions and their genuine inverse relation.
#[derive(Default)]
pub(crate) struct QualityRows<'source> {
    pub(crate) distributions: Vec<QualityDistributionDefinition<'source>>,
    pub(crate) quantizations: Vec<QualityQuantizationDefinition<'source>>,
    pub(crate) inverse: Vec<QualityQuantizationResources>,
    pub(crate) uses: Vec<QuantizationUse>,
    pub(crate) summary: QuantizationSummary,
    pub(crate) skipped_distributions: usize,
    pub(crate) skipped_quantizations: usize,
}

/// Internal eligibility and relationship facts used by Commodity fallback.
pub(crate) struct QuantizationUse {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_index: u32,
    pub(crate) resource_type_guids: Vec<CatalogGuid>,
}
