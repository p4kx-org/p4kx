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

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct StarmapLocation<'source> {
    source_type: SourcedValue<&'source str>,
    set_entity_location_on_enter: SourcedValue<bool>,
    expose_for_player_created_missions: SourcedValue<bool>,
    exclude_from_level_load: SourcedValue<bool>,
    authored_fields: SourceArenaOutput<'source>,
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

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct StarmapQuantumTravel<'source> {
    source_type: SourcedValue<&'source str>,
    obstruction_radius: SourcedValue<ResourceFloat32>,
    arrival_radius: SourcedValue<ResourceFloat32>,
    arrival_point_detection_offset: SourcedValue<ResourceFloat32>,
    adoption_radius: SourcedValue<ResourceFloat32>,
    sub_point_radius_multiplier: SourcedValue<ResourceFloat32>,
    authored_fields: SourceArenaOutput<'source>,
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
