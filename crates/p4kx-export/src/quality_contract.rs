// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::guid_text::CatalogGuid;
use crate::quality_source::{
    QualityCurveSource, QualityOverridesSource, QualityQuantizationSource, QualitySource,
};
use crate::schema_fields::every_field_required;

/// Stable private identity captured directly from the original successful Record tree.
#[derive(Debug, Clone, Copy)]
pub(crate) struct QualityOwner<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_index: u32,
    pub(crate) record_path: &'source str,
}

/// Authored distribution and location-override declarations of one physical Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct QualityDistributionDefinition<'source> {
    #[serde(skip)]
    pub(crate) owner: QualityOwner<'source>,
    #[serde(skip)]
    pub(crate) numbers: Vec<CapturedNumber>,
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) quality_distribution: SourcedValue<QualitySource<'source, QualityCurveSource>>,
    pub(crate) location_override:
        SourcedValue<QualitySource<'source, QualityOverridesSource<'source>>>,
}

/// Authored quantization declarations, independent of inverse eligibility.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct QualityQuantizationDefinition<'source> {
    #[serde(skip)]
    pub(crate) owner: QualityOwner<'source>,
    #[serde(skip)]
    pub(crate) numbers: Vec<CapturedNumber>,
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) quality_quantization:
        SourcedValue<QualitySource<'source, QualityQuantizationSource<'source>>>,
}

/// The narrow eligibility result used by the nominal-GUID inverse relation.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(tag = "status", content = "reason", rename_all = "camelCase")]
pub(crate) enum QuantizationEligibility {
    Eligible,
    NotEligible(QuantizationIneligibility),
}

/// Original source decisions that exclude a complete raw definition from inverse lookup.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum QuantizationIneligibility {
    NoInlineBands,
    EmptyBands,
    InvalidBands,
    Unavailable,
}

/// A first-eligible nominal-GUID inverse without a second authored quality payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct QualityQuantizationResources {
    #[serde(skip)]
    pub(crate) physical_guid: CatalogGuid,
    #[serde(skip)]
    pub(crate) physical_index: u32,
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_index: SourcedValue<u32>,
    pub(crate) eligibility: SourcedValue<QuantizationEligibility>,
    pub(crate) resource_type_guids: SourcedValue<Vec<SourcedValue<CatalogGuid>>>,
}
