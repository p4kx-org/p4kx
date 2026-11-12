// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::catalog_item_alignment::PhysicalPort;
use super::network_endpoint::NetworkEndpoint;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::loadout::alignment::AttachmentIdentity;
use crate::schema_fields::every_field_required;

#[derive(Default)]
pub(crate) struct ResourceNetworkPlacements {
    pub(super) nodes: Vec<ResourceNetworkNode>,
    pub(super) links: Vec<ResourceNetworkLinkBinding>,
    pub(super) rooms: Vec<RoomAssociationBinding>,
}

impl ResourceNetworkPlacements {
    pub(crate) fn into_tables(self, forge: &DataForge<'_>) -> CatalogPlacements {
        CatalogPlacements {
            resource_network_nodes: CatalogTable::new(
                forge,
                &[
                    "ItemResourceComponentParams",
                    "SInternalHardpointLink",
                    "SInternalResourceLink",
                ],
                self.nodes,
            ),
            resource_network_link_bindings: CatalogTable::new(
                forge,
                &["SInternalHardpointLink", "SInternalResourceLink"],
                self.links,
            ),
            room_association_bindings: CatalogTable::new(
                forge,
                &["SVehicleObjectContainerParams"],
                self.rooms,
            ),
        }
    }
}

/// Contextual placements referring to authored definitions and declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogPlacements {
    resource_network_nodes: CatalogTable<ResourceNetworkNode>,
    resource_network_link_bindings: CatalogTable<ResourceNetworkLinkBinding>,
    room_association_bindings: CatalogTable<RoomAssociationBinding>,
}

/// A position in the published ship Loadout and its nested port arrays.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct LoadoutPosition {
    pub(super) loadout_ordinal: u32,
    pub(super) port_ordinals: Vec<u32>,
}

/// A contextual port or ship root backed by an installed or authored network fact.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ResourceNetworkNode {
    pub(super) position: LoadoutPosition,
    pub(super) original_port: Option<PhysicalPort>,
    pub(super) attached_item: Option<AttachmentIdentity>,
    pub(super) vehicle_boundary: Option<SourcedValue<bool>>,
    pub(super) resource_components: Vec<InstalledResourceComponent>,
    pub(super) fuse_evidence: FuseEvidence,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(super) enum FuseEvidence {
    Unestablished,
}

/// The selected physical resource component and its original relay discriminator.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InstalledResourceComponent {
    #[serde(flatten)]
    pub(super) owner: ComponentOwner,
    pub(super) is_relay: SourcedValue<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(super) enum LinkFamily {
    Hardpoint,
    Resource,
}

/// One original link applied to a single declaring Loadout context.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ResourceNetworkLinkBinding {
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
pub(super) struct RoomAssociationBinding {
    pub(super) context: LoadoutPosition,
    #[serde(flatten)]
    pub(super) owner: ComponentOwner,
    pub(super) container_ordinal: SourcedValue<u32>,
    pub(super) port: SourcedValue<NetworkEndpoint>,
}
