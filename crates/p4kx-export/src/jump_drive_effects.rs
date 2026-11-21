// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::quantum_fields::QuantumFields;
use super::source_getters;
use crate::CatalogError;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// Original jump-drive presentation parameters grouped by travel phase.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct JumpDriveEffects<'source> {
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

/// Authored jump-drive exit settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct JumpDriveExit<'source> {
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

/// Authored jump-drive opening settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct JumpDriveOpening<'source> {
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

/// Authored jump-drive travel settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct JumpDriveTravel<'source> {
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

/// Authored jump-drive wake settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct JumpDriveWake<'source> {
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

/// The original endpoints of a jump-drive effect range.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct JumpDriveEffectRange<'source> {
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

impl<'source> JumpDriveEffects<'source> {
    source_getters! {
        /// The original concrete source type.
        source_type: SourcedValue<&'source str>,
        /// The authored exit parameters.
        exit: SourcedValue<JumpDriveExit<'source>>,
        /// The authored opening parameters.
        opening: SourcedValue<JumpDriveOpening<'source>>,
        /// The authored travel parameters.
        travel: SourcedValue<JumpDriveTravel<'source>>,
        /// The authored wake parameters.
        wake: SourcedValue<JumpDriveWake<'source>>,
        /// The original projectile effect declaration.
        projectile_effect: SourcedValue<SourceArenaOutput<'source>>,
        /// The original tunnel response effect declaration.
        tunnel_response_effect: SourcedValue<SourceArenaOutput<'source>>,
        /// The original particle declarations.
        jump_drive_particles: SourcedValue<SourceArenaOutput<'source>>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> JumpDriveExit<'source> {
    source_getters! {
        /// The authored range of exit speeds.
        exit_speed_range: SourcedValue<JumpDriveEffectRange<'source>>,
        /// The authored exit trail time.
        exit_trail_time: SourcedValue<ResourceNumber>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> JumpDriveOpening<'source> {
    source_getters! {
        /// The authored shield pulse maximum time.
        shield_pulse_max_time: SourcedValue<ResourceNumber>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> JumpDriveTravel<'source> {
    source_getters! {
        /// The authored interaction start distance.
        interaction_start_distance: SourcedValue<ResourceNumber>,
        /// The authored reaction burst trigger distance.
        reaction_burst_trigger_distance: SourcedValue<ResourceNumber>,
        /// The authored tunnel response ahead range.
        tunnel_response_distance_ahead_range: SourcedValue<JumpDriveEffectRange<'source>>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> JumpDriveWake<'source> {
    source_getters! {
        /// The authored maximum penetration.
        max_penetration: SourcedValue<ResourceNumber>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> JumpDriveEffectRange<'source> {
    source_getters! {
        /// The original minimum endpoint.
        minimum: SourcedValue<ResourceNumber>,
        /// The original maximum endpoint.
        maximum: SourcedValue<ResourceNumber>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}
