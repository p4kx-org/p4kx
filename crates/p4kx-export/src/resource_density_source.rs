// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract;

/// A resource's authored density or molar-mass source.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceDensitySource<'source> {
    /// The concrete `ResourceTypeDensityType` source struct.
    pub(crate) source_type: &'source str,
    /// The corresponding stored payload.
    pub(crate) payload: ResourceDensityPayload<'source>,
}

/// The distinct physical quantities the source can declare.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ResourceDensityPayload<'source> {
    /// A mass density with its authored unit.
    Density(ResourceDensity<'source>),
    /// A molar mass whose source declaration gives no unit label.
    MolarMass(ResourceMolarMass),
    /// The source is the known fieldless density base.
    EmptyBase,
    /// The source has an unsupported concrete subtype and blocking Export health.
    Unsupported,
}

/// The unit-bearing density payload of a `ResourceTypeDensity`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceDensity<'source> {
    /// The authored `densityUnit`, without unit conversion.
    pub(crate) density_unit: Option<ResourceDensityUnit<'source>>,
}

/// A concrete authored density unit and its raw value.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceDensityUnit<'source> {
    /// The concrete source unit struct.
    pub(crate) source_type: &'source str,
    /// The source unit and its corresponding value.
    pub(crate) payload: ResourceDensityUnitPayload,
}

/// Density values in their authored units.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ResourceDensityUnitPayload {
    /// A value in grams per cubic centimetre.
    GramsPerCubicCentimeter(ResourceDensityValue),
    /// A value in kilograms per cubic metre.
    KilogramsPerCubicMeter(ResourceDensityValue),
    /// The source is the known fieldless density-unit base.
    EmptyBase,
    /// The source has an unsupported concrete unit and blocking Export health.
    Unsupported,
}

/// An authored finite density value without normalization.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceDensityValue {
    /// The raw density, null only for an absent or null source field.
    pub(crate) value: Option<f32>,
}

/// An authored molar mass without an inferred unit.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceMolarMass {
    /// The raw `molarMass`, null only for an absent or null source field.
    pub(crate) molar_mass: Option<f32>,
}
