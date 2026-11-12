// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const VEHICLE: &str = "VehicleComponentParams";
const OBJECT_CONTAINER: &str = "SVehicleObjectContainerParams";

/// Original room attachment declarations for a physical vehicle component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RoomAssociations<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    object_containers: SourcedValue<Vec<SourcedValue<RoomAssociation<'source>>>>,
}

impl<'source> RoomAssociations<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            owner,
            object_containers: source.class_array_to(
                "objectContainers",
                OBJECT_CONTAINER,
                notes,
                RoomAssociation::read,
            )?,
        })
    }

    pub(super) fn containers(&self) -> impl Iterator<Item = &RoomAssociation<'source>> {
        self.object_containers
            .value()
            .into_iter()
            .flatten()
            .filter_map(SourcedValue::value)
    }

    pub(super) const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct RoomAssociation<'source> {
    pub(super) container_ordinal: SourcedValue<u32>,
    #[serde(skip)]
    pub(super) port_use: Option<SourceUse>,
    pub(super) resource_network_itemport_name: SourcedValue<&'source str>,
    bone_name: SourcedValue<&'source str>,
    file_name: SourcedValue<&'source str>,
    guid: SourcedValue<&'source str>,
    should_request_nav_mesh_regeneration_on_load: SourcedValue<bool>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> RoomAssociation<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let resource_network_itemport_name =
            source.borrowed_string("resourceNetworkItemportName", notes)?;
        let bone_name = source.borrowed_string("boneName", notes)?;
        let file_name = source.borrowed_string("fileName", notes)?;
        let guid = source.borrowed_string("guid", notes)?;
        let should_request_nav_mesh_regeneration_on_load =
            source.boolean("shouldRequestNavMeshRegenerationOnLoad", notes)?;
        let additional_fields = source.authored_members_except([
            (
                "resourceNetworkItemportName",
                resource_network_itemport_name.state(),
            ),
            ("boneName", bone_name.state()),
            ("fileName", file_name.state()),
            ("guid", guid.state()),
            (
                "shouldRequestNavMeshRegenerationOnLoad",
                should_request_nav_mesh_regeneration_on_load.state(),
            ),
        ])?;
        Ok(Self {
            container_ordinal: source.owned_element_ordinal()?,
            port_use: source.selector_use("resourceNetworkItemportName")?,
            resource_network_itemport_name,
            bone_name,
            file_name,
            guid,
            should_request_nav_mesh_regeneration_on_load,
            additional_fields,
        })
    }
}
