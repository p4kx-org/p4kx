// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::quantum_fields::QuantumFields;
use crate::CatalogError;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// One authored quantum drive mode with its independent configuration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct QuantumProfile<'source> {
    source_type: SourcedValue<&'source str>,
    drive_speed: SourcedValue<ResourceNumber>,
    cooldown_time: SourcedValue<ResourceNumber>,
    spool_up_time: SourcedValue<ResourceNumber>,
    stage_one_accel_rate: SourcedValue<ResourceNumber>,
    stage_two_accel_rate: SourcedValue<ResourceNumber>,
    engage_speed: SourcedValue<ResourceNumber>,
    calibration_rate: SourcedValue<ResourceNumber>,
    min_calibration_requirement: SourcedValue<ResourceNumber>,
    max_calibration_requirement: SourcedValue<ResourceNumber>,
    calibration_process_angle_limit: SourcedValue<ResourceNumber>,
    calibration_warning_angle_limit: SourcedValue<ResourceNumber>,
    calibration_delay_in_seconds: SourcedValue<ResourceNumber>,
    interdiction_effect_time: SourcedValue<ResourceNumber>,
    vfx_spool_end_velocity: SourcedValue<ResourceNumber>,
    vfx_pinch_effect_time: SourcedValue<ResourceNumber>,
    vfx_pinch_max_velocity: SourcedValue<ResourceNumber>,
    vfx_entry_flash_velocity: SourcedValue<ResourceNumber>,
    vfx_trail_start_velocity: SourcedValue<ResourceNumber>,
    vfx_travel_effect_start_velocity: SourcedValue<ResourceNumber>,
    vfx_travel_effect_end_velocity: SourcedValue<ResourceNumber>,
    vfx_exit_effect_velocity: SourcedValue<ResourceNumber>,
    shader_node_engage_velocity: SourcedValue<ResourceNumber>,
    shader_node_shutdown_velocity: SourcedValue<ResourceNumber>,
    shader_node_max_strength_velocity: SourcedValue<ResourceNumber>,
    idle_state: SourcedValue<SourceArenaOutput<'source>>,
    travelling_state: SourcedValue<SourceArenaOutput<'source>>,
    effect_tags: SourcedValue<SourceArenaOutput<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumProfile<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            source_type: fields.source_type()?,
            drive_speed: fields.number("driveSpeed")?,
            cooldown_time: fields.number("cooldownTime")?,
            spool_up_time: fields.number("spoolUpTime")?,
            stage_one_accel_rate: fields.number("stageOneAccelRate")?,
            stage_two_accel_rate: fields.number("stageTwoAccelRate")?,
            engage_speed: fields.number("engageSpeed")?,
            calibration_rate: fields.number("calibrationRate")?,
            min_calibration_requirement: fields.number("minCalibrationRequirement")?,
            max_calibration_requirement: fields.number("maxCalibrationRequirement")?,
            calibration_process_angle_limit: fields.number("calibrationProcessAngleLimit")?,
            calibration_warning_angle_limit: fields.number("calibrationWarningAngleLimit")?,
            calibration_delay_in_seconds: fields.number("calibrationDelayInSeconds")?,
            interdiction_effect_time: fields.number("interdictionEffectTime")?,
            vfx_spool_end_velocity: fields.number("VFXSpoolEndVelocity")?,
            vfx_pinch_effect_time: fields.number("VFXPinchEffectTime")?,
            vfx_pinch_max_velocity: fields.number("VFXPinchMaxVelocity")?,
            vfx_entry_flash_velocity: fields.number("VFXEntryFlashVelocity")?,
            vfx_trail_start_velocity: fields.number("VFXTrailStartVelocity")?,
            vfx_travel_effect_start_velocity: fields.number("VFXTravelEffectStartVelocity")?,
            vfx_travel_effect_end_velocity: fields.number("VFXTravelEffectEndVelocity")?,
            vfx_exit_effect_velocity: fields.number("VFXExitEffectVelocity")?,
            shader_node_engage_velocity: fields.number("ShaderNodeEngageVelocity")?,
            shader_node_shutdown_velocity: fields.number("ShaderNodeShutdownVelocity")?,
            shader_node_max_strength_velocity: fields.number("ShaderNodeMaxStrengthVelocity")?,
            idle_state: fields.member("idleState")?,
            travelling_state: fields.member("travellingState")?,
            effect_tags: fields.member("effectTags")?,
            additional_fields: fields.additional_fields()?,
        })
    }
}
