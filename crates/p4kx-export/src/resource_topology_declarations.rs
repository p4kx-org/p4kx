// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use super::resource_network_links::{self, ResourceNetworkLinks};
use super::room_associations::{self, RoomAssociations};
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::export_field_inspection::ConfigurationInspection;
use crate::source_fields::FieldError;

#[path = "item_room_association.rs"]
mod item_room;

pub(super) use item_room::COMPONENT as ITEM_ROOM;
pub use item_room::ItemRoomAssociation;

#[derive(Clone, Copy)]
pub(crate) enum ResourceTopologyKind {
    Links,
    VehicleRooms,
    ItemRooms,
}

impl ResourceTopologyKind {
    pub(crate) const fn source_types(self) -> &'static [&'static str] {
        match self {
            Self::Links => &[resource_network_links::PORT_CONTAINER],
            Self::VehicleRooms => &[room_associations::VEHICLE],
            Self::ItemRooms => &[ITEM_ROOM],
        }
    }

    pub(super) const fn inspection(self) -> ConfigurationInspection {
        match self {
            Self::Links => ConfigurationInspection::ResourceNetworkLinks,
            Self::VehicleRooms => ConfigurationInspection::VehicleRooms,
            Self::ItemRooms => ConfigurationInspection::ItemRooms,
        }
    }

    pub(super) const fn inspected_members(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Links => Some(resource_network_links::MEMBERS),
            Self::VehicleRooms => Some(&[room_associations::OBJECT_CONTAINERS]),
            Self::ItemRooms => None,
        }
    }
}

#[derive(Default)]
pub(crate) struct ResourceTopologyDeclarations<'source> {
    pub(super) links: Vec<ResourceNetworkLinks<'source>>,
    pub(super) rooms: Vec<RoomAssociations<'source>>,
    pub(super) item_rooms: Vec<ItemRoomAssociation<'source>>,
}

impl<'source> ResourceTopologyDeclarations<'source> {
    pub(super) fn project(
        &mut self,
        kind: ResourceTopologyKind,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        match kind {
            ResourceTopologyKind::Links => {
                if let Some(row) = ResourceNetworkLinks::read(owner, source, notes)? {
                    self.links.push(row);
                }
            }
            ResourceTopologyKind::VehicleRooms => {
                self.rooms
                    .push(RoomAssociations::read(owner, source, notes)?);
            }
            ResourceTopologyKind::ItemRooms => {
                self.item_rooms
                    .push(ItemRoomAssociation::read(owner, source, notes)?);
            }
        }
        Ok(())
    }

    pub(crate) fn into_links(self) -> Vec<ResourceNetworkLinks<'source>> {
        self.links
    }

    pub(crate) fn into_vehicle_rooms(self) -> Vec<RoomAssociations<'source>> {
        self.rooms
    }

    pub(crate) fn into_item_rooms(self) -> Vec<ItemRoomAssociation<'source>> {
        self.item_rooms
    }

    pub(crate) const fn has_context_rows(&self) -> bool {
        !self.links.is_empty() || !self.rooms.is_empty()
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.links.extend(other.links);
        self.rooms.extend(other.rooms);
        self.item_rooms.extend(other.item_rooms);
    }

    pub(crate) fn into_tables(mut self, forge: &DataForge<'_>) -> ResourceTopologyTables<'source> {
        self.links.sort_by_key(|row| row.owner().key());
        self.rooms.sort_by_key(|row| row.owner().key());
        self.item_rooms.sort_by_key(|row| row.owner().key());
        ResourceTopologyTables {
            resource_network_links: CatalogTable::new(
                forge,
                ResourceTopologyKind::Links.source_types(),
                self.links,
            ),
            room_associations: CatalogTable::new(
                forge,
                ResourceTopologyKind::VehicleRooms.source_types(),
                self.rooms,
            ),
            item_room_associations: CatalogTable::new(
                forge,
                ResourceTopologyKind::ItemRooms.source_types(),
                self.item_rooms,
            ),
        }
    }
}

pub(crate) struct ResourceTopologyTables<'source> {
    pub(crate) resource_network_links: CatalogTable<ResourceNetworkLinks<'source>>,
    pub(crate) room_associations: CatalogTable<RoomAssociations<'source>>,
    pub(crate) item_room_associations: CatalogTable<ItemRoomAssociation<'source>>,
}
