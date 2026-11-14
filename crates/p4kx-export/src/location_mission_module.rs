// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::super::authored_fields::Observed;
use super::super::super::mission_sources::module_property::ModuleMissionProperty;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[path = "location_mission_module_input.rs"]
mod input;

use input::MissionModuleInput;

/// One authored module occurrence in the original mission location.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct LocationMissionModule<'source> {
    is_persistent: SourcedValue<bool>,
    mission_module: SourcedValue<&'source str>,
    mission_inputs: SourcedValue<Vec<SourcedValue<MissionModuleInput<'source>>>>,
    properties: SourcedValue<Vec<SourcedValue<ModuleMissionProperty<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> LocationMissionModule<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let is_persistent = fields.boolean("isPersistent", notes)?;
        let mission_module = fields.borrowed_string("missionModule", notes)?;
        let mission_inputs = MissionModuleInput::read_array(fields, notes)?;
        let properties = fields.class_array_to(
            "properties",
            "MissionProperty",
            notes,
            ModuleMissionProperty::read,
        )?;
        let authored_fields = fields.authored_members_except([
            ("isPersistent", is_persistent.state()),
            ("missionModule", mission_module.state()),
            ("missionInputs", mission_inputs.state()),
            ("properties", properties.state()),
        ])?;
        Ok(Self {
            is_persistent,
            mission_module,
            mission_inputs,
            properties,
            authored_fields,
        })
    }
}
