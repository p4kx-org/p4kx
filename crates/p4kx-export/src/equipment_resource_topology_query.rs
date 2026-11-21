// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::record_query;
use super::rows::{ConfigurationFamily, Family, Projector, ReadOptions};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_entity_configurations::{
    self, ResourceTopologyDeclarations, ResourceTopologyKind,
};
use crate::record_observation::RecordObservation;

pub use crate::catalog_v3::catalog_entity_configurations::{
    HardpointLink, ItemRoomAssociation, ResourceNetworkLinks, ResourcePortLink, RoomAssociation,
    RoomAssociations,
};

macro_rules! topology_projector {
    ($type:ident, $row:ident, $kind:ident, $rows:ident) => {
        struct $type;

        impl Projector for $type {
            type Row<'source> = $row<'source>;
            const FAMILY: Family = Family::Configuration(ConfigurationFamily::ResourceTopology(
                ResourceTopologyKind::$kind,
            ));

            fn read<'source>(
                _: &'source DataForge<'source>,
                tree: &RecordTree<'source>,
                projection: &CatalogProjection<'_, 'source>,
                admission: &RecordObservation<'_, 'source>,
                _: &ReadOptions<'_>,
                cancel: Option<&CancelToken>,
            ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
                let read = catalog_entity_configurations::read_resource_topology(
                    tree,
                    projection,
                    admission,
                    ResourceTopologyKind::$kind,
                    cancel,
                )?;
                Ok(read.map(ResourceTopologyDeclarations::$rows))
            }
        }
    };
}

topology_projector!(Links, ResourceNetworkLinks, Links, into_links);
topology_projector!(
    VehicleRooms,
    RoomAssociations,
    VehicleRooms,
    into_vehicle_rooms
);
topology_projector!(ItemRooms, ItemRoomAssociation, ItemRooms, into_item_rooms);

record_query::unlocalized!(
    Links,
    ResourceNetworkLinks,
    /// Original ordered resource link declarations and their source evidence.
    ResourceNetworkLinksSnapshot,
    with_resource_network_links,
    with_resource_network_links_from
);

record_query::unlocalized!(
    VehicleRooms,
    RoomAssociations,
    /// Original vehicle room declarations and their source evidence.
    RoomAssociationsSnapshot,
    with_room_associations,
    with_room_associations_from
);

record_query::unlocalized!(
    ItemRooms,
    ItemRoomAssociation,
    /// Original item room declarations and their source evidence.
    ItemRoomAssociationsSnapshot,
    with_item_room_associations,
    with_item_room_associations_from
);
