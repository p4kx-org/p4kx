// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::contract::{
    QualityDistributionDefinition, QualityQuantizationDefinition, QualityQuantizationResources,
};
use super::fallbacks::Fallbacks;
use crate::QuantizationSummary;

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
