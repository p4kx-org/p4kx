// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Direct canonical quality definitions and selective inverse resource membership.

#[path = "quality_query_api.rs"]
mod api;
#[path = "quality_query_family.rs"]
mod family;
#[path = "quality_inverse_query.rs"]
mod inverse;
#[path = "quality_query_read.rs"]
mod read;
#[path = "quality_query_rows.rs"]
mod rows;
#[path = "quality_query_snapshot.rs"]
mod snapshot;

pub use super::{CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability};
pub use crate::canonical_record_header::CanonicalRecordHeader;
pub use crate::catalog_record_reference::CatalogRecordReference;
pub use crate::definition_reference::DefinitionStatus;
pub use crate::guid_text::CatalogGuid;
pub use crate::quality_catalog::contract::{
    QualityDistributionDefinition, QualityQuantizationDefinition, QualityQuantizationResources,
    QuantizationEligibility, QuantizationIneligibility,
};
pub use crate::quality_source::{
    InlineQuality, QualityBandSource, QualityCurveSource, QualityOverrideFields,
    QualityOverridePayload, QualityOverrideSource, QualityOverridesSource, QualityPayload,
    QualityQuantizationSource, QualityRecordReference, QualitySource,
};
pub use crate::resource_float::{ResourceFloat32, ResourceFloat64};
pub use crate::resource_integer::ResourceInt64;
pub use crate::resource_number::ResourceNumber;
pub use api::{
    with_distributions, with_distributions_from, with_quantizations, with_quantizations_from,
};
pub use inverse::{
    QuantizationResourceSnapshot, with_quantization_resources, with_quantization_resources_from,
};
pub use snapshot::{DistributionSnapshot, QualityDefinitionSnapshot, QuantizationSnapshot};

#[cfg(test)]
#[path = "quality_query_tests.rs"]
mod tests;
