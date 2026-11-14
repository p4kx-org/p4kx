// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::quantum_fields::QuantumFields;
use crate::CatalogError;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// Original jump-drive presentation parameters grouped by travel phase.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct JumpDriveEffects<'source> {
    source_type: SourcedValue<&'source str>,
    exit: SourcedValue<JumpDriveExit<'source>>,
    opening: SourcedValue<JumpDriveOpening<'source>>,
    travel: SourcedValue<JumpDriveTravel<'source>>,
    wake: SourcedValue<JumpDriveWake<'source>>,
    projectile_effect: SourcedValue<SourceArenaOutput<'source>>,
    tunnel_response_effect: SourcedValue<SourceArenaOutput<'source>>,
    jump_drive_particles: SourcedValue<SourceArenaOutput<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> JumpDriveEffects<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            source_type: fields.source_type()?,
            exit: fields.class("exitParams", JumpDriveExit::read)?,
            opening: fields.class("openingParams", JumpDriveOpening::read)?,
            travel: fields.class("travelParams", JumpDriveTravel::read)?,
            wake: fields.class("wakeParams", JumpDriveWake::read)?,
            projectile_effect: fields.member("projectileEffect")?,
            tunnel_response_effect: fields.member("tunnelResponseEffect")?,
            jump_drive_particles: fields.member("jumpDriveParticles")?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct JumpDriveExit<'source> {
    exit_speed_range: SourcedValue<JumpDriveEffectRange<'source>>,
    exit_trail_time: SourcedValue<ResourceNumber>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> JumpDriveExit<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            exit_speed_range: fields.class("exitSpeedRange", JumpDriveEffectRange::read)?,
            exit_trail_time: fields.number("exitTrailTime")?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct JumpDriveOpening<'source> {
    shield_pulse_max_time: SourcedValue<ResourceNumber>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> JumpDriveOpening<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            shield_pulse_max_time: fields.number("shieldPulseMaxTime")?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct JumpDriveTravel<'source> {
    interaction_start_distance: SourcedValue<ResourceNumber>,
    reaction_burst_trigger_distance: SourcedValue<ResourceNumber>,
    tunnel_response_distance_ahead_range: SourcedValue<JumpDriveEffectRange<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> JumpDriveTravel<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            interaction_start_distance: fields.number("interactionStartDistance")?,
            reaction_burst_trigger_distance: fields.number("reactionBurstTriggerDistance")?,
            tunnel_response_distance_ahead_range: fields.class(
                "tunnelResponseDistanceAheadRange",
                JumpDriveEffectRange::read,
            )?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct JumpDriveWake<'source> {
    max_penetration: SourcedValue<ResourceNumber>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> JumpDriveWake<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            max_penetration: fields.number("maxPenetration")?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct JumpDriveEffectRange<'source> {
    minimum: SourcedValue<ResourceNumber>,
    maximum: SourcedValue<ResourceNumber>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> JumpDriveEffectRange<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            minimum: fields.number("minimum")?,
            maximum: fields.number("maximum")?,
            additional_fields: fields.additional_fields()?,
        })
    }
}
