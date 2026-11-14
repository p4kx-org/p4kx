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
pub(crate) struct ResourceDensitySource<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<ResourceDensityPayload<'source>>,
}

/// The closed physical quantity shapes accepted from the source.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ResourceDensityPayload<'source> {
    Density(ResourceDensity<'source>),
    MolarMass(ResourceMolarMass),
    EmptyBase,
}

/// A density retaining its authored unit without conversion.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceDensity<'source> {
    pub(crate) density_unit: SourcedValue<ResourceDensityUnit<'source>>,
}

/// A concrete authored density unit and its corresponding source value.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceDensityUnit<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<ResourceDensityUnitPayload>,
}

/// The closed density-unit shapes preserve their declared quantities.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ResourceDensityUnitPayload {
    GramsPerCubicCentimeter(ResourceDensityValue),
    KilogramsPerCubicMeter(ResourceDensityValue),
    EmptyBase,
}

/// An exact authored Single density without normalization.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceDensityValue {
    pub(crate) value: SourcedValue<ResourceFloat32>,
}

/// An exact authored molar mass without an inferred unit.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceMolarMass {
    pub(crate) molar_mass: SourcedValue<ResourceFloat32>,
}
