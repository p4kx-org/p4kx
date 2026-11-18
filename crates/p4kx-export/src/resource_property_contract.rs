// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_record_reference::CatalogRecordReference;
use crate::quality_source::{
    QualityCurveSource, QualityOverridesSource, QualityQuantizationSource, QualitySource,
};
use crate::resource_float::ResourceFloat32;
use crate::schema_fields;

/// One named authored cargo slot without a derived capacity.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ResourceCargoSlot<'source> {
    pub(crate) slot: SourcedValue<&'source str>,
    pub(crate) entity_class: SourcedValue<CatalogRecordReference>,
}

/// One authored resource property and its exact concrete source type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ResourceProperty<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) name: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<ResourcePropertyPayload<'source>>,
}

/// The closed property payload shapes accepted from resource declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ResourcePropertyPayload<'source> {
    /// An authored base property without additional values.
    Base,
    /// The source volatility and health-decay values.
    Volatility(ResourceVolatility),
    /// The independently authored crafting-quality sources.
    CraftingData(Box<ResourceCraftingData<'source>>),
}

/// Raw source volatility values, including all legal IEEE representations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ResourceVolatility {
    pub(crate) volatility: SourcedValue<ResourceFloat32>,
    pub(crate) health_decay_per_second: SourcedValue<ResourceFloat32>,
}

/// Independently authored quality configuration sources.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ResourceCraftingData<'source> {
    #[serde(rename = "qualityDistribution")]
    pub(crate) distribution: SourcedValue<QualitySource<'source, QualityCurveSource>>,
    #[serde(rename = "qualityLocationOverride")]
    pub(crate) location_override:
        SourcedValue<QualitySource<'source, QualityOverridesSource<'source>>>,
    #[serde(rename = "qualityQuantization")]
    pub(crate) quantization:
        SourcedValue<QualitySource<'source, QualityQuantizationSource<'source>>>,
}

impl<'source> ResourceCargoSlot<'source> {
    /// Borrows the authored cargo-slot name.
    #[must_use]
    pub const fn slot(&self) -> &SourcedValue<&'source str> {
        &self.slot
    }

    /// Borrows the original cargo-container Entity reference.
    #[must_use]
    pub const fn entity_class(&self) -> &SourcedValue<CatalogRecordReference> {
        &self.entity_class
    }
}

impl<'source> ResourceProperty<'source> {
    /// Borrows the original concrete resource-property type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the authored property name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<&'source str> {
        &self.name
    }

    /// Borrows the original property variant and its immediate source state.
    #[must_use]
    pub const fn payload(&self) -> &SourcedValue<ResourcePropertyPayload<'source>> {
        &self.payload
    }
}

impl ResourceVolatility {
    /// Borrows the source volatility without changing its IEEE representation.
    #[must_use]
    pub const fn volatility(&self) -> &SourcedValue<ResourceFloat32> {
        &self.volatility
    }

    /// Borrows the authored health decay per second.
    #[must_use]
    pub const fn health_decay_per_second(&self) -> &SourcedValue<ResourceFloat32> {
        &self.health_decay_per_second
    }
}

impl<'source> ResourceCraftingData<'source> {
    /// Borrows the authored inline, referenced or empty quality distribution.
    #[must_use]
    pub const fn quality_distribution(
        &self,
    ) -> &SourcedValue<QualitySource<'source, QualityCurveSource>> {
        &self.distribution
    }

    /// Borrows the original quality location-override source.
    #[must_use]
    pub const fn quality_location_override(
        &self,
    ) -> &SourcedValue<QualitySource<'source, QualityOverridesSource<'source>>> {
        &self.location_override
    }

    /// Borrows the ordered authored quality-quantization source.
    #[must_use]
    pub const fn quality_quantization(
        &self,
    ) -> &SourcedValue<QualitySource<'source, QualityQuantizationSource<'source>>> {
        &self.quantization
    }
}
