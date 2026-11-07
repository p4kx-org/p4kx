// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{self, LocalisedText};
use crate::contract_rewards::QualityCurve;
use crate::definition_reference::DefinitionReference;
use crate::guid_text::CatalogGuid;
use crate::resource_density_source::ResourceDensitySource;
use crate::resource_quality_source::{
    ResourceQualityOverrides, ResourceQualityQuantization, ResourceQualitySource,
};

/// A selected `ResourceType` Record, independently of the commodities that refer to it.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceType<'source> {
    /// The persistent source Record GUID.
    pub(crate) guid: CatalogGuid,
    /// The source Record name without its type prefix.
    pub(crate) class_name: &'source str,
    /// The source Record path without `.xml`.
    pub(crate) record_path: &'source str,
    /// The localised `displayName`.
    pub(crate) name: LocalisedText<'source>,
    /// The localised source description.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: LocalisedText<'source>,
    /// All selected groups that directly list this resource, distinct and ordered by GUID.
    pub(crate) group_guids: Vec<CatalogGuid>,
    /// The authored refined-resource link, including an unresolved GUID.
    pub(crate) refined_version: Option<DefinitionReference>,
    /// The authored density or molar mass with its concrete source type.
    pub(crate) density: Option<ResourceDensitySource<'source>>,
    /// The raw thumbnail path, including a stored empty string.
    pub(crate) default_thumbnail_path: Option<&'source str>,
    /// The raw SVG thumbnail path, including a stored empty string.
    #[serde(rename = "defaultThumbnailPathSVG")]
    pub(crate) default_thumbnail_path_svg: Option<&'source str>,
    /// The source Entity Reference used for an RTT thumbnail.
    pub(crate) rtt_thumbnail_entity_class: Option<DefinitionReference>,
    /// The authored validation flag; false differs from absent or null.
    pub(crate) validate_default_cargo_box: Option<bool>,
    /// Every authored named cargo slot in source order, without inferred capacities.
    pub(crate) default_cargo_containers: Option<Vec<ResourceCargoSlot<'source>>>,
    /// Every property in source order, retaining null entries and repeated names.
    pub(crate) properties: Option<Vec<Option<ResourceProperty<'source>>>>,
}

/// A named field of the resource's default cargo-container declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceCargoSlot<'source> {
    /// The exact authored field name, without interpretation as a capacity.
    pub(crate) slot: &'source str,
    /// The authored Entity Reference or an explicit null slot.
    pub(crate) entity_class: Option<DefinitionReference>,
}

/// One authored `ResourceTypeProperties` value.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceProperty<'source> {
    /// The concrete source property struct.
    pub(crate) source_type: &'source str,
    /// The inherited raw `name`, including an empty string.
    pub(crate) name: Option<&'source str>,
    /// The concrete declared fields or explicit unsupported state.
    pub(crate) payload: ResourcePropertyPayload<'source>,
}

/// The fields authored by one resource-property subtype.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ResourcePropertyPayload<'source> {
    /// The base property carries only its inherited name.
    Base,
    /// Raw volatility and health-decay fields.
    Volatility(ResourceVolatility),
    /// Independent quality distribution, location and quantization sources.
    CraftingData(Box<ResourceCraftingData<'source>>),
    /// The concrete subtype is retained with a blocking Export health diagnostic.
    Unsupported,
}

/// The declared fields of a `ResourceTypeVolatility` property.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceVolatility {
    /// The raw `volatility` without clamping.
    pub(crate) volatility: Option<f32>,
    /// The raw `healthDecayPerSecond` without clamping.
    pub(crate) health_decay_per_second: Option<f32>,
}

/// The independent authored quality sources of a resource crafting property.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceCraftingData<'source> {
    /// The distribution source, preserving inline and Record-linked forms.
    #[serde(rename = "qualityDistribution")]
    pub(crate) distribution: Option<ResourceQualitySource<'source, QualityCurve<'source>>>,
    /// The location-override source, preserving the source entry list.
    #[serde(rename = "qualityLocationOverride")]
    pub(crate) location_override:
        Option<ResourceQualitySource<'source, ResourceQualityOverrides<'source>>>,
    /// The quantization source, sharing exact-integer band payloads.
    #[serde(rename = "qualityQuantization")]
    pub(crate) quantization: Option<ResourceQualitySource<'source, ResourceQualityQuantization>>,
}
