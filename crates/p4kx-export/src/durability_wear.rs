// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::observed::Observed;
use crate::CatalogError;
use crate::catalog_v3::authored_effect_resource::Rtpc;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Original wear-specific fields of an accumulator.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct WearFields {
    max_lifetime_hours: SourcedValue<ResourceFloat32>,
    use_as_timer: SourcedValue<bool>,
    initial_age_ratio: SourcedValue<ResourceFloat32>,
    initial_usage_ratio: SourcedValue<ResourceFloat32>,
    atmosphere_multiplier: SourcedValue<ResourceFloat32>,
    damage_conversion_rate: SourcedValue<ResourceFloat32>,
    heat_multipliers: SourcedValue<HeatMultipliers>,
    functionality_multiplier: SourcedValue<FunctionalityMultiplier>,
    effect_curve: SourcedValue<DegradationCurve>,
    degrade_from_parent_params: SourcedValue<DegradeFromParent>,
    degradation_percentage_rtpc: SourcedValue<Rtpc>,
}

impl WearFields {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            max_lifetime_hours: source.single("MaxLifetimeHours", notes)?,
            use_as_timer: source.boolean("UseAsTimer", notes)?,
            initial_age_ratio: source.single("InitialAgeRatio", notes)?,
            initial_usage_ratio: source.single("InitialUsageRatio", notes)?,
            atmosphere_multiplier: source.single("AtmosphereMultiplier", notes)?,
            damage_conversion_rate: source.single("DamageConversionRate", notes)?,
            heat_multipliers: source.owned(
                "HeatMultipliers",
                &["HeatDegradation"],
                notes,
                HeatMultipliers::read,
            )?,
            functionality_multiplier: source.owned(
                "FunctionalityMultiplier",
                &["FunctionalityDegradation"],
                notes,
                FunctionalityMultiplier::read,
            )?,
            effect_curve: source.owned(
                "EffectCurve",
                &["BezierCurve"],
                notes,
                DegradationCurve::read,
            )?,
            degrade_from_parent_params: source.owned(
                "degradeFromParentParams",
                &["DegradeFromParentParams"],
                notes,
                DegradeFromParent::read,
            )?,
            degradation_percentage_rtpc: source.owned(
                "DegradationPercentageRTPC",
                &["AudioRtpc"],
                notes,
                Rtpc::read,
            )?,
        })
    }
}

/// Authored temperature multiplier words.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HeatMultipliers {
    normal_temperature_multiplier: SourcedValue<ResourceFloat32>,
    overheat_temperature_multiplier: SourcedValue<ResourceFloat32>,
}

impl HeatMultipliers {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            normal_temperature_multiplier: source.single("NormalTemperatureMultiplier", notes)?,
            overheat_temperature_multiplier: source
                .single("OverheatTemperatureMultiplier", notes)?,
        })
    }
}

/// The authored power-ratio flag and optional functionality curve.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct FunctionalityMultiplier {
    use_power_ratio: SourcedValue<bool>,
    functionality_multiply_curve: SourcedValue<DegradationCurve>,
}

impl FunctionalityMultiplier {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            use_power_ratio: source.boolean("UsePowerRatio", notes)?,
            functionality_multiply_curve: source.owned(
                "FunctionalityMultiplyCurve",
                &["BezierCurve"],
                notes,
                DegradationCurve::read,
            )?,
        })
    }
}

/// Authored parent-resource degradation inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DegradeFromParent {
    degrade_one_at_a_time: SourcedValue<bool>,
    resource_delta_type: SourcedValue<String>,
    resource_type: SourcedValue<String>,
    resource_to_degradation_rate: SourcedValue<ResourceFloat32>,
}

impl DegradeFromParent {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            degrade_one_at_a_time: source.boolean("degradeOneAtATime", notes)?,
            resource_delta_type: source.enumeration("resourceDeltaType", notes)?,
            resource_type: source.enumeration("resourceType", notes)?,
            resource_to_degradation_rate: source.single("resourceToDegradationRate", notes)?,
        })
    }
}

/// The original curve controls and authored point slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DegradationCurve {
    use_lut: SourcedValue<bool>,
    points: SourcedValue<Vec<SourcedValue<CurvePoint>>>,
}

impl DegradationCurve {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            use_lut: source.boolean("useLUT", notes)?,
            points: source.array("points", &["Vec2"], notes, CurvePoint::read)?,
        })
    }
}

/// The exact stored coordinates of an authored curve point.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CurvePoint {
    x: SourcedValue<ResourceFloat32>,
    y: SourcedValue<ResourceFloat32>,
}

impl CurvePoint {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            x: source.single("x", notes)?,
            y: source.single("y", notes)?,
        })
    }
}
