// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const LOCATION_TYPE: &str = "StarMapObjectLocationParams";
pub(super) const TRAVEL_TYPE: &str = "StarMapQuantumTravelDataParams";

/// Authored location handling for a starmap object or preset.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct StarmapLocation<'source> {
    /// The original concrete type.
    pub source_type: SourcedValue<&'source str>,
    /// Whether to set the entity location on entry.
    pub set_entity_location_on_enter: SourcedValue<bool>,
    /// Whether to expose the location for player-created missions.
    pub expose_for_player_created_missions: SourcedValue<bool>,
    /// Whether to exclude this location from level loading.
    pub exclude_from_level_load: SourcedValue<bool>,
    /// Additional original fields.
    pub authored_fields: SourceArenaOutput<'source>,
}

impl<'source> StarmapLocation<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let set_entity_location_on_enter = fields.boolean("setEntityLocationOnEnter", notes)?;
        let expose_for_player_created_missions =
            fields.boolean("exposeForPlayerCreatedMissions", notes)?;
        let exclude_from_level_load = fields.boolean("excludeFromLevelLoad", notes)?;
        let authored_fields = fields.authored_members_except([
            (
                "setEntityLocationOnEnter",
                set_entity_location_on_enter.state(),
            ),
            (
                "exposeForPlayerCreatedMissions",
                expose_for_player_created_missions.state(),
            ),
            ("excludeFromLevelLoad", exclude_from_level_load.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            set_entity_location_on_enter,
            expose_for_player_created_missions,
            exclude_from_level_load,
            authored_fields,
        })
    }
}

/// Authored starmap travel radii and offsets.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct StarmapQuantumTravel<'source> {
    /// The original concrete type.
    pub source_type: SourcedValue<&'source str>,
    /// The authored obstruction radius.
    pub obstruction_radius: SourcedValue<ResourceFloat32>,
    /// The authored arrival radius.
    pub arrival_radius: SourcedValue<ResourceFloat32>,
    /// The authored arrival-point detection offset.
    pub arrival_point_detection_offset: SourcedValue<ResourceFloat32>,
    /// The authored adoption radius.
    pub adoption_radius: SourcedValue<ResourceFloat32>,
    /// The authored sub-point radius multiplier.
    pub sub_point_radius_multiplier: SourcedValue<ResourceFloat32>,
    /// Additional original fields.
    pub authored_fields: SourceArenaOutput<'source>,
}

impl<'source> StarmapQuantumTravel<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let obstruction_radius = fields.single("obstructionRadius", notes)?;
        let arrival_radius = fields.single("arrivalRadius", notes)?;
        let arrival_point_detection_offset = fields.single("arrivalPointDetectionOffset", notes)?;
        let adoption_radius = fields.single("adoptionRadius", notes)?;
        let sub_point_radius_multiplier = fields.single("subPointRadiusMultiplier", notes)?;
        let authored_fields = fields.authored_members_except([
            ("obstructionRadius", obstruction_radius.state()),
            ("arrivalRadius", arrival_radius.state()),
            (
                "arrivalPointDetectionOffset",
                arrival_point_detection_offset.state(),
            ),
            ("adoptionRadius", adoption_radius.state()),
            (
                "subPointRadiusMultiplier",
                sub_point_radius_multiplier.state(),
            ),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            obstruction_radius,
            arrival_radius,
            arrival_point_detection_offset,
            adoption_radius,
            sub_point_radius_multiplier,
            authored_fields,
        })
    }
}
