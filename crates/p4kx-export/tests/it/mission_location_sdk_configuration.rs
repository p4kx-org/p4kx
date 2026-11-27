// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    LocationMissionLimit, LocationMissionModule, MissionLocationConfiguration,
    ModuleMissionProperty, ModuleMissionPropertyValue,
};
use serde_json::{Value, json};

use super::{TestResult, proof::View};

impl View<'_> {
    pub(super) fn configuration(
        &mut self,
        value: &MissionLocationConfiguration<'_>,
    ) -> TestResult<Value> {
        Ok(json!({
            "missionModules": self.sequence(value.mission_modules(), Self::module)?,
            "autoSpawnSettings": self.sequence(value.auto_spawn_settings(), Self::spawn)?,
            "missionLimits": self.sequence(value.mission_limits(), Self::mission_limit)?,
            "defaultSecurityNetworkManifest": self.cell(value.default_security_network_manifest(),
                |view, value| view.reference(value))?,
        }))
    }

    fn mission_limit(&mut self, value: &LocationMissionLimit<'_>) -> TestResult<Value> {
        Ok(json!({
            "tag": self.cell(value.tag(), |view, value| view.reference(value))?,
            "autoConsumeIfPlayerAtLocation": self.scalar(value.auto_consume_if_player_at_location())?,
            "maxMissionInstances": self.scalar(value.max_mission_instances())?,
            "authoredFields": self.arena(value.authored_fields())?,
        }))
    }

    fn module(&mut self, value: &LocationMissionModule<'_>) -> TestResult<Value> {
        Ok(json!({
            "isPersistent": self.scalar(value.is_persistent())?,
            "missionModule": self.scalar(value.mission_module())?,
            "missionInputs": self.sequence(value.mission_inputs(), Self::input)?,
            "properties": self.sequence(value.properties(), Self::property)?,
            "authoredFields": self.arena(value.authored_fields())?,
        }))
    }

    fn property(&mut self, value: &ModuleMissionProperty<'_>) -> TestResult<Value> {
        Ok(json!({
            "missionVariableName": self.scalar(value.mission_variable_name())?,
            "extendedTextToken": self.scalar(value.extended_text_token())?,
            "value": self.cell(value.value(), Self::property_value)?,
            "authoredFields": self.arena(value.authored_fields())?,
        }))
    }

    fn property_value(&mut self, value: &ModuleMissionPropertyValue<'_>) -> TestResult<Value> {
        Ok(json!({
            "sourceType": self.scalar(value.source_type())?,
            "authoredFields": self.arena(value.authored_fields())?,
        }))
    }
}
