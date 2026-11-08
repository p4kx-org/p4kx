// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::resource_float::ResourceFloat32;
use crate::resource_quality_source::{
    ResourceQualityCurve, ResourceQualityOverrides, ResourceQualityQuantization,
    ResourceQualitySource,
};
use crate::resource_reference::ResourceReference;
use crate::schema_fields;

/// One named authored cargo slot without a derived capacity.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceCargoSlot<'source> {
    pub(crate) slot: SourcedValue<&'source str>,
    pub(crate) entity_class: SourcedValue<ResourceReference>,
}

/// One authored resource property and its exact concrete source type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceProperty<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) name: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<ResourcePropertyPayload<'source>>,
}

/// The closed property payload shapes accepted from resource declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ResourcePropertyPayload<'source> {
    Base,
    Volatility(ResourceVolatility),
    CraftingData(Box<ResourceCraftingData<'source>>),
}

/// Raw source volatility values, including all legal IEEE representations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceVolatility {
    pub(crate) volatility: SourcedValue<ResourceFloat32>,
    pub(crate) health_decay_per_second: SourcedValue<ResourceFloat32>,
}

/// Independently authored quality configuration sources.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceCraftingData<'source> {
    #[serde(rename = "qualityDistribution")]
    pub(crate) distribution:
        SourcedValue<ResourceQualitySource<'source, ResourceQualityCurve<'source>>>,
    #[serde(rename = "qualityLocationOverride")]
    pub(crate) location_override:
        SourcedValue<ResourceQualitySource<'source, ResourceQualityOverrides<'source>>>,
    #[serde(rename = "qualityQuantization")]
    pub(crate) quantization:
        SourcedValue<ResourceQualitySource<'source, ResourceQualityQuantization<'source>>>,
}
