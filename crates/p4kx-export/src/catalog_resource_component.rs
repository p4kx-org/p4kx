// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_resource_control_parameter::{self, ResourceControlParameter};
use super::catalog_resource_state::ResourceState;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const COMPONENT: &str = "ItemResourceComponentParams";

type AuthoredBlock<'source> = SourcedValue<SourceArenaOutput<'source>>;

/// Resource-network parameters and ordered states of one physical component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ResourceComponent<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    is_resource_networked: SourcedValue<bool>,
    is_relay: SourcedValue<bool>,
    is_connected_to_room: SourcedValue<bool>,
    wireless_connection: SourcedValue<bool>,
    default_priority: SourcedValue<i32>,
    repair_restore_ratio: SourcedValue<ResourceFloat32>,
    states: SourcedValue<Vec<SourcedValue<ResourceState<'source>>>>,
    self_repair: SourcedValue<SelfRepair<'source>>,
    #[serde(flatten)]
    controls: ResourceControls<'source>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourceComponent<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let is_resource_networked = source.boolean("isResourceNetworked", notes)?;
        let is_relay = source.boolean("isRelay", notes)?;
        let is_connected_to_room = source.boolean("isConnectedToRoom", notes)?;
        let wireless_connection = source.boolean("wirelessConnection", notes)?;
        let default_priority = source.int32("defaultPriority", notes)?;
        let repair_restore_ratio = source.single("repairRestoreRatio", notes)?;
        let states = source.declared_class_array("states", notes, ResourceState::read)?;
        let self_repair = source.declared_class("selfRepair", notes, SelfRepair::read)?;
        let controls = ResourceControls::read(source, notes)?;
        let additional_fields = source.authored_members_except(
            [
                ("isResourceNetworked", is_resource_networked.state()),
                ("isRelay", is_relay.state()),
                ("isConnectedToRoom", is_connected_to_room.state()),
                ("wirelessConnection", wireless_connection.state()),
                ("defaultPriority", default_priority.state()),
                ("repairRestoreRatio", repair_restore_ratio.state()),
                ("states", states.state()),
                ("selfRepair", self_repair.state()),
            ]
            .into_iter()
            .chain(controls.projected()),
        )?;
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            is_resource_networked,
            is_relay,
            is_connected_to_room,
            wireless_connection,
            default_priority,
            repair_restore_ratio,
            states,
            self_repair,
            controls,
            additional_fields,
        })
    }

    pub(super) const fn is_relay(&self) -> &SourcedValue<bool> {
        &self.is_relay
    }

    pub(super) const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ResourceControls<'source> {
    filter_params: AuthoredBlock<'source>,
    control_parameters: SourcedValue<Vec<SourcedValue<ResourceControlParameter<'source>>>>,
    control_blocks: AuthoredBlock<'source>,
    functionality_modifiers: AuthoredBlock<'source>,
    power_plant_override: AuthoredBlock<'source>,
}

impl<'source> ResourceControls<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            filter_params: source.authored_member("filterParams", notes)?,
            control_parameters: source.strong_pointer_family_array_to(
                "controlParameters",
                catalog_resource_control_parameter::BASE,
                notes,
                ResourceControlParameter::read,
            )?,
            control_blocks: source.authored_member("controlBlocks", notes)?,
            functionality_modifiers: source.authored_member("functionalityModifiers", notes)?,
            power_plant_override: source.authored_member("powerPlantOverride", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 5] {
        [
            ("filterParams", self.filter_params.state()),
            ("controlParameters", self.control_parameters.state()),
            ("controlBlocks", self.control_blocks.state()),
            (
                "functionalityModifiers",
                self.functionality_modifiers.state(),
            ),
            ("powerPlantOverride", self.power_plant_override.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SelfRepair<'source> {
    source_type: SourcedValue<&'source str>,
    max_repair_count: SourcedValue<i32>,
    time_to_repair: SourcedValue<ResourceFloat32>,
    health_ratio: SourcedValue<ResourceFloat32>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> SelfRepair<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let max_repair_count = source.int32("maxRepairCount", notes)?;
        let time_to_repair = source.single("timeToRepair", notes)?;
        let health_ratio = source.single("healthRatio", notes)?;
        let additional_fields = source.authored_members_except([
            ("maxRepairCount", max_repair_count.state()),
            ("timeToRepair", time_to_repair.state()),
            ("healthRatio", health_ratio.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            max_repair_count,
            time_to_repair,
            health_ratio,
            additional_fields,
        })
    }
}
