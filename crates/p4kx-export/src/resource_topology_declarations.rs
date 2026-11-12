// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;

use super::resource_network_links::{self, ResourceNetworkLinks};
use super::room_associations::{self, RoomAssociations};
use crate::catalog_table::CatalogTable;

#[derive(Default)]
pub(crate) struct ResourceTopologyDeclarations<'source> {
    pub(super) links: Vec<ResourceNetworkLinks<'source>>,
    pub(super) rooms: Vec<RoomAssociations<'source>>,
}

impl<'source> ResourceTopologyDeclarations<'source> {
    pub(super) fn extend(&mut self, other: Self) {
        self.links.extend(other.links);
        self.rooms.extend(other.rooms);
    }

    pub(crate) fn into_tables(mut self, forge: &DataForge<'_>) -> ResourceTopologyTables<'source> {
        self.links.sort_by_key(|row| row.owner().key());
        self.rooms.sort_by_key(|row| row.owner().key());
        ResourceTopologyTables {
            resource_network_links: CatalogTable::new(
                forge,
                &[resource_network_links::PORT_CONTAINER],
                self.links,
            ),
            room_associations: CatalogTable::new(forge, &[room_associations::VEHICLE], self.rooms),
        }
    }
}

pub(crate) struct ResourceTopologyTables<'source> {
    pub(crate) resource_network_links: CatalogTable<ResourceNetworkLinks<'source>>,
    pub(crate) room_associations: CatalogTable<RoomAssociations<'source>>,
}
