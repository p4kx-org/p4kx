// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::resource_float::ResourceFloat32;
use crate::schema_fields;

/// A resource's authored density or molar-mass source.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ResourceDensitySource<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<ResourceDensityPayload<'source>>,
}

/// The closed physical quantity shapes accepted from the source.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ResourceDensityPayload<'source> {
    /// Density using the authored unit.
    Density(ResourceDensity<'source>),
    /// Molar mass without an inferred unit.
    MolarMass(ResourceMolarMass),
    /// An authored base source without a quantity.
    EmptyBase,
}

/// A density retaining its authored unit without conversion.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ResourceDensity<'source> {
    pub(crate) density_unit: SourcedValue<ResourceDensityUnit<'source>>,
}

/// A concrete authored density unit and its corresponding source value.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ResourceDensityUnit<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<ResourceDensityUnitPayload>,
}

/// The closed density-unit shapes preserve their declared quantities.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ResourceDensityUnitPayload {
    /// The source quantity in grams per cubic centimetre.
    GramsPerCubicCentimeter(ResourceDensityValue),
    /// The source quantity in kilograms per cubic metre.
    KilogramsPerCubicMeter(ResourceDensityValue),
    /// An authored base unit without a quantity.
    EmptyBase,
}

/// An exact authored Single density without normalization.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ResourceDensityValue {
    pub(crate) value: SourcedValue<ResourceFloat32>,
}

/// An exact authored molar mass without an inferred unit.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ResourceMolarMass {
    pub(crate) molar_mass: SourcedValue<ResourceFloat32>,
}

impl<'source> ResourceDensitySource<'source> {
    /// Borrows the original concrete density-source type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the density payload with its original source state.
    #[must_use]
    pub const fn payload(&self) -> &SourcedValue<ResourceDensityPayload<'source>> {
        &self.payload
    }
}

impl<'source> ResourceDensity<'source> {
    /// Borrows the authored unit without converting its quantity.
    #[must_use]
    pub const fn density_unit(&self) -> &SourcedValue<ResourceDensityUnit<'source>> {
        &self.density_unit
    }
}

impl<'source> ResourceDensityUnit<'source> {
    /// Borrows the original concrete density-unit type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the exact authored unit variant and its quantity.
    #[must_use]
    pub const fn payload(&self) -> &SourcedValue<ResourceDensityUnitPayload> {
        &self.payload
    }
}

impl ResourceDensityValue {
    /// Borrows the source Single with its original IEEE bits.
    #[must_use]
    pub const fn value(&self) -> &SourcedValue<ResourceFloat32> {
        &self.value
    }
}

impl ResourceMolarMass {
    /// Borrows the authored molar mass without assigning a unit.
    #[must_use]
    pub const fn molar_mass(&self) -> &SourcedValue<ResourceFloat32> {
        &self.molar_mass
    }
}
