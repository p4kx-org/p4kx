// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::OrderedCurve;
use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_resource_flow_unit::ResourceFlowUnit;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct IfcsFuelUsage<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    atmospheric_curve_multiplier: SourcedValue<ResourceFloat32>,
    atmospheric_modifier: SourcedValue<OrderedCurve<'source>>,
    fuel_cost_per10_k_newton: SourcedValue<ResourceFlowUnit<'source>>,
    idle_cost: SourcedValue<ResourceFlowUnit<'source>>,
    no_fuel_threshold_ratio: SourcedValue<ResourceFloat32>,
}

impl<'source> IfcsFuelUsage<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let atmospheric_curve_multiplier = fields.single("atmosphericCurveMultiplier", notes)?;
        let atmospheric_modifier =
            fields.declared_class("atmosphericModifier", notes, OrderedCurve::read)?;
        let fuel_cost_per10_k_newton = fields.strong_pointer_family_to(
            "fuelCostPer10KNewton",
            "SBaseResourceUnit",
            notes,
            ResourceFlowUnit::read,
        )?;
        let idle_cost = fields.strong_pointer_family_to(
            "idleCost",
            "SBaseResourceUnit",
            notes,
            ResourceFlowUnit::read,
        )?;
        let no_fuel_threshold_ratio = fields.single("noFuelThresholdRatio", notes)?;
        let source = UnitSource::read(
            fields,
            [
                (
                    "atmosphericCurveMultiplier",
                    atmospheric_curve_multiplier.state(),
                ),
                ("atmosphericModifier", atmospheric_modifier.state()),
                ("fuelCostPer10KNewton", fuel_cost_per10_k_newton.state()),
                ("idleCost", idle_cost.state()),
                ("noFuelThresholdRatio", no_fuel_threshold_ratio.state()),
            ],
        )?;
        Ok(Self {
            source,
            atmospheric_curve_multiplier,
            atmospheric_modifier,
            fuel_cost_per10_k_newton,
            idle_cost,
            no_fuel_threshold_ratio,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct IfcsNoFuel<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    angular_acceleration_modifier: SourcedValue<ResourceFloat32>,
    angular_velocity_modifier: SourcedValue<ResourceFloat32>,
    linear_acceleration_modifier: SourcedValue<ResourceFloat32>,
    linear_max_speed: SourcedValue<ResourceFloat32>,
}

impl<'source> IfcsNoFuel<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let angular_acceleration_modifier = fields.single("angularAccelerationModifier", notes)?;
        let angular_velocity_modifier = fields.single("angularVelocityModifier", notes)?;
        let linear_acceleration_modifier = fields.single("linearAccelerationModifier", notes)?;
        let linear_max_speed = fields.single("linearMaxSpeed", notes)?;
        let source = UnitSource::read(
            fields,
            [
                (
                    "angularAccelerationModifier",
                    angular_acceleration_modifier.state(),
                ),
                ("angularVelocityModifier", angular_velocity_modifier.state()),
                (
                    "linearAccelerationModifier",
                    linear_acceleration_modifier.state(),
                ),
                ("linearMaxSpeed", linear_max_speed.state()),
            ],
        )?;
        Ok(Self {
            source,
            angular_acceleration_modifier,
            angular_velocity_modifier,
            linear_acceleration_modifier,
            linear_max_speed,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct IfcsThrusterEfficiency<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    atmospheric_efficiency_curve: SourcedValue<OrderedCurve<'source>>,
    max_supported_atmospheric_efficiency: SourcedValue<ResourceFloat32>,
}

impl<'source> IfcsThrusterEfficiency<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let atmospheric_efficiency_curve =
            fields.declared_class("atmosphericEfficiencyCurve", notes, OrderedCurve::read)?;
        let max_supported_atmospheric_efficiency =
            fields.single("maxSupportedAtmosphericEfficiency", notes)?;
        let source = UnitSource::read(
            fields,
            [
                (
                    "atmosphericEfficiencyCurve",
                    atmospheric_efficiency_curve.state(),
                ),
                (
                    "maxSupportedAtmosphericEfficiency",
                    max_supported_atmospheric_efficiency.state(),
                ),
            ],
        )?;
        Ok(Self {
            source,
            atmospheric_efficiency_curve,
            max_supported_atmospheric_efficiency,
        })
    }
}
