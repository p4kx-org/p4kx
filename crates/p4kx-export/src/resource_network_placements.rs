// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

pub use super::catalog_item_alignment::PhysicalPort;
pub use super::loadout_position::LoadoutPosition;
pub use super::network_endpoint::{EndpointCandidate, NetworkEndpoint};
use crate::catalog_rows::spool::{RowSpool, RowSpoolError};
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::loadout::alignment::AttachmentIdentity;
use crate::schema_fields::every_field_required;

#[path = "resource_network_placement_api.rs"]
mod api;
pub use api::PlacementTable;

const LINK_TYPES: &[&str] = &["SInternalHardpointLink", "SInternalResourceLink"];

#[derive(Default)]
pub(crate) struct ResourceNetworkPlacements {
    pub(super) nodes: Vec<ResourceNetworkNode>,
    pub(super) rooms: Vec<RoomAssociationBinding>,
}

impl ResourceNetworkPlacements {
    pub(super) fn into_tables(
        self,
        forge: &DataForge<'_>,
        links: Vec<ResourceNetworkLinkBinding>,
    ) -> ResourceNetworkPlacementTables {
        let (resource_network_nodes, room_association_bindings) = self.tables(forge);
        ResourceNetworkPlacementTables {
            resource_network_nodes,
            resource_network_link_bindings: CatalogTable::new(forge, LINK_TYPES, links),
            room_association_bindings,
        }
    }

    pub(super) fn into_staged_tables(
        self,
        forge: &DataForge<'_>,
        links: Option<RowSpool>,
        cancel: Option<&CancelToken>,
    ) -> StagedResourceNetworkPlacementTables {
        let availability = if links.is_some() {
            DatasetAvailability::Available
        } else {
            DatasetAvailability::from_source_declarations(forge, |name| LINK_TYPES.contains(&name))
        };
        let (resource_network_nodes, room_association_bindings) = self.tables(forge);
        StagedResourceNetworkPlacementTables {
            resource_network_nodes,
            resource_network_link_bindings: StagedCatalogTable::new(availability, links, cancel),
            room_association_bindings,
        }
    }

    fn tables(
        self,
        forge: &DataForge<'_>,
    ) -> (
        CatalogTable<ResourceNetworkNode>,
        CatalogTable<RoomAssociationBinding>,
    ) {
        let nodes = CatalogTable::new(
            forge,
            &[
                "ItemResourceComponentParams",
                "SInternalHardpointLink",
                "SInternalResourceLink",
            ],
            self.nodes,
        );
        let rooms = CatalogTable::new(forge, &["SVehicleObjectContainerParams"], self.rooms);
        (nodes, rooms)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StagedResourceNetworkPlacementTables {
    resource_network_nodes: CatalogTable<ResourceNetworkNode>,
    resource_network_link_bindings: StagedCatalogTable,
    room_association_bindings: CatalogTable<RoomAssociationBinding>,
}

impl StagedResourceNetworkPlacementTables {
    pub(crate) const fn stored_bytes(&self) -> u64 {
        self.resource_network_link_bindings.stored_bytes()
    }

    pub(crate) fn take_failure(&self) -> Option<RowSpoolError> {
        self.resource_network_link_bindings.take_failure()
    }
}

/// Contextual placements referring to authored definitions and declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourceNetworkPlacementTables {
    resource_network_nodes: CatalogTable<ResourceNetworkNode>,
    resource_network_link_bindings: CatalogTable<ResourceNetworkLinkBinding>,
    room_association_bindings: CatalogTable<RoomAssociationBinding>,
}

/// A contextual port or ship root backed by an installed or authored network fact.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourceNetworkNode {
    pub(super) position: LoadoutPosition,
    pub(super) original_port: Option<PhysicalPort>,
    pub(super) attached_item: Option<AttachmentIdentity>,
    pub(super) vehicle_boundary: Option<SourcedValue<bool>>,
    pub(super) resource_components: Vec<InstalledResourceComponent>,
    pub(super) fuse_evidence: FuseEvidence,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(description = "", extend("x-p4kx-vocabularyClass" = "control"))]
#[expect(
    missing_docs,
    reason = "variant docs would change the shared schema representation"
)]
#[non_exhaustive]
/// The evidence that establishes an installed node's fuse behavior.
pub enum FuseEvidence {
    Unestablished,
}

/// The selected physical resource component and its original relay discriminator.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct InstalledResourceComponent {
    #[serde(flatten)]
    pub(super) owner: ComponentOwner,
    pub(super) is_relay: SourcedValue<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(description = "", extend("x-p4kx-vocabularyClass" = "control"))]
#[expect(
    missing_docs,
    reason = "variant docs would change the shared schema representation"
)]
#[non_exhaustive]
/// The original named-hardpoint or weak resource link family.
pub enum LinkFamily {
    Hardpoint,
    Resource,
}

/// One original link applied to a single declaring Loadout context.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourceNetworkLinkBinding {
    pub(super) context: LoadoutPosition,
    pub(super) family: LinkFamily,
    #[serde(flatten)]
    pub(super) owner: ComponentOwner,
    pub(super) link_ordinal: SourcedValue<u32>,
    pub(super) port1: SourcedValue<NetworkEndpoint>,
    pub(super) port2: SourcedValue<NetworkEndpoint>,
}

/// One original room declaration applied to a single declaring Loadout context.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct RoomAssociationBinding {
    pub(super) context: LoadoutPosition,
    #[serde(flatten)]
    pub(super) owner: ComponentOwner,
    pub(super) container_ordinal: SourcedValue<u32>,
    pub(super) port: SourcedValue<NetworkEndpoint>,
}
