// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

use super::missile_shake::MissileAxes;

const PHASE: &str = "SGuidanceAndControlPhaseParams";

/// Original guidance and control settings with independently authored phases.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MissileGuidance<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    is_dumb_missile: SourcedValue<bool>,
    dumbfire_rotation_scale: SourcedValue<ResourceFloat32>,
    linear_speed: SourcedValue<ResourceFloat32>,
    boost_phase_duration: SourcedValue<ResourceFloat32>,
    terminal_phase_engagement_time: SourcedValue<ResourceFloat32>,
    terminal_phase_engagement_angle: SourcedValue<ResourceFloat32>,
    fuel_tank_size: SourcedValue<ResourceFloat32>,
    pid_integral_term: SourcedValue<ResourceFloat32>,
    pid_derivative_term: SourcedValue<ResourceFloat32>,
    pid_proportional_term: SourcedValue<ResourceFloat32>,
    boost_phase: SourcedValue<MissileGuidancePhase<'source>>,
    intercept_phase: SourcedValue<MissileGuidancePhase<'source>>,
    terminal_phase: SourcedValue<MissileGuidancePhase<'source>>,
}

impl<'source> MissileGuidance<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let is_dumb_missile = source.boolean("isDumbMissile", notes)?;
        let dumbfire_rotation_scale = source.single("dumbfireRotationScale", notes)?;
        let linear_speed = source.single("linearSpeed", notes)?;
        let boost_phase_duration = source.single("boostPhaseDuration", notes)?;
        let terminal_phase_engagement_time = source.single("terminalPhaseEngagementTime", notes)?;
        let terminal_phase_engagement_angle =
            source.single("terminalPhaseEngagementAngle", notes)?;
        let fuel_tank_size = source.single("fuelTankSize", notes)?;
        let pid_integral_term = source.single("pidIntegralTerm", notes)?;
        let pid_derivative_term = source.single("pidDerivativeTerm", notes)?;
        let pid_proportional_term = source.single("pidProportionalTerm", notes)?;
        let boost_phase =
            source.declared_class_to("boostPhase", PHASE, notes, MissileGuidancePhase::read)?;
        let intercept_phase =
            source.declared_class_to("interceptPhase", PHASE, notes, MissileGuidancePhase::read)?;
        let terminal_phase =
            source.declared_class_to("terminalPhase", PHASE, notes, MissileGuidancePhase::read)?;
        let source = UnitSource::read(
            source,
            [
                ("isDumbMissile", is_dumb_missile.state()),
                ("dumbfireRotationScale", dumbfire_rotation_scale.state()),
                ("linearSpeed", linear_speed.state()),
                ("boostPhaseDuration", boost_phase_duration.state()),
                (
                    "terminalPhaseEngagementTime",
                    terminal_phase_engagement_time.state(),
                ),
                (
                    "terminalPhaseEngagementAngle",
                    terminal_phase_engagement_angle.state(),
                ),
                ("fuelTankSize", fuel_tank_size.state()),
                ("pidIntegralTerm", pid_integral_term.state()),
                ("pidDerivativeTerm", pid_derivative_term.state()),
                ("pidProportionalTerm", pid_proportional_term.state()),
                ("boostPhase", boost_phase.state()),
                ("interceptPhase", intercept_phase.state()),
                ("terminalPhase", terminal_phase.state()),
            ],
        )?;
        Ok(Self {
            source,
            is_dumb_missile,
            dumbfire_rotation_scale,
            linear_speed,
            boost_phase_duration,
            terminal_phase_engagement_time,
            terminal_phase_engagement_angle,
            fuel_tank_size,
            pid_integral_term,
            pid_derivative_term,
            pid_proportional_term,
            boost_phase,
            intercept_phase,
            terminal_phase,
        })
    }
}

/// Original controller settings and acceleration vectors for one guidance phase.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct MissileGuidancePhase<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    angular_speed: SourcedValue<ResourceFloat32>,
    max_rotation_accel: SourcedValue<ResourceFloat32>,
    pid_aggression: SourcedValue<ResourceFloat32>,
    max_linear_acceleration_positive: SourcedValue<MissileAxes<'source>>,
    max_linear_acceleration_negative: SourcedValue<MissileAxes<'source>>,
}

impl<'source> MissileGuidancePhase<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let angular_speed = source.single("angularSpeed", notes)?;
        let max_rotation_accel = source.single("maxRotationAccel", notes)?;
        let pid_aggression = source.single("pidAggression", notes)?;
        let max_linear_acceleration_positive = source.declared_class_to(
            "maxLinearAccelerationPositive",
            "Vec3",
            notes,
            MissileAxes::read,
        )?;
        let max_linear_acceleration_negative = source.declared_class_to(
            "maxLinearAccelerationNegative",
            "Vec3",
            notes,
            MissileAxes::read,
        )?;
        let source = UnitSource::read(
            source,
            [
                ("angularSpeed", angular_speed.state()),
                ("maxRotationAccel", max_rotation_accel.state()),
                ("pidAggression", pid_aggression.state()),
                (
                    "maxLinearAccelerationPositive",
                    max_linear_acceleration_positive.state(),
                ),
                (
                    "maxLinearAccelerationNegative",
                    max_linear_acceleration_negative.state(),
                ),
            ],
        )?;
        Ok(Self {
            source,
            angular_speed,
            max_rotation_accel,
            pid_aggression,
            max_linear_acceleration_positive,
            max_linear_acceleration_negative,
        })
    }
}
