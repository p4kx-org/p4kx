// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;

use super::{
    AttachmentIdentity, FuseEvidence, InstalledResourceComponent, LinkFamily, LoadoutPosition,
    NetworkEndpoint, PhysicalPort, ResourceNetworkLinkBinding, ResourceNetworkNode,
    ResourceNetworkPlacementTables, RoomAssociationBinding,
};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::catalog_v3::item_component_owner::ComponentOwner;

/// A borrowed placement table and its original declaration availability.
#[derive(Debug)]
pub struct PlacementTable<'rows, Row> {
    /// Availability of the original declaration family.
    pub availability: DatasetAvailability,
    /// Selected contextual rows in Catalog order.
    pub rows: &'rows [Row],
}

impl<'rows, Row> PlacementTable<'rows, Row> {
    fn new(table: &'rows CatalogTable<Row>) -> Self {
        Self {
            availability: table.availability,
            rows: &table.rows,
        }
    }
}

impl ResourceNetworkPlacementTables {
    /// Borrows the installed and authored resource-network nodes.
    #[must_use]
    pub fn resource_network_nodes(&self) -> PlacementTable<'_, ResourceNetworkNode> {
        PlacementTable::new(&self.resource_network_nodes)
    }

    /// Borrows the contextual bindings of authored network links.
    #[must_use]
    pub fn resource_network_link_bindings(&self) -> PlacementTable<'_, ResourceNetworkLinkBinding> {
        PlacementTable::new(&self.resource_network_link_bindings)
    }

    /// Borrows the contextual bindings of authored vehicle room selectors.
    #[must_use]
    pub fn room_association_bindings(&self) -> PlacementTable<'_, RoomAssociationBinding> {
        PlacementTable::new(&self.room_association_bindings)
    }
}

impl ResourceNetworkNode {
    /// Borrows the physical root and nested port route.
    #[must_use]
    pub const fn position(&self) -> &LoadoutPosition {
        &self.position
    }

    /// Returns the original physical declaration of the node's port.
    #[must_use]
    pub const fn original_port(&self) -> Option<PhysicalPort> {
        self.original_port
    }

    /// Borrows the selected physical item attachment.
    #[must_use]
    pub const fn attached_item(&self) -> Option<&AttachmentIdentity> {
        self.attached_item.as_ref()
    }

    /// Borrows the sourced indication that the attachment is a docked vehicle.
    #[must_use]
    pub const fn vehicle_boundary(&self) -> Option<&SourcedValue<bool>> {
        self.vehicle_boundary.as_ref()
    }

    /// Borrows the selected resources and their original component identities.
    #[must_use]
    pub fn resource_components(&self) -> &[InstalledResourceComponent] {
        &self.resource_components
    }

    /// Borrows the recorded fuse evidence.
    #[must_use]
    pub const fn fuse_evidence(&self) -> &FuseEvidence {
        &self.fuse_evidence
    }
}

impl InstalledResourceComponent {
    /// Borrows the original owner for a resource component.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the original sourced relay discriminator.
    #[must_use]
    pub const fn is_relay(&self) -> &SourcedValue<bool> {
        &self.is_relay
    }
}

impl ResourceNetworkLinkBinding {
    /// Borrows the physical context that declares the link.
    #[must_use]
    pub const fn context(&self) -> &LoadoutPosition {
        &self.context
    }

    /// Borrows the declaration family.
    #[must_use]
    pub const fn family(&self) -> &LinkFamily {
        &self.family
    }

    /// Borrows the original component that declares the link.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the original link ordinal within its component and family.
    #[must_use]
    pub const fn link_ordinal(&self) -> &SourcedValue<u32> {
        &self.link_ordinal
    }

    /// Borrows the first original endpoint's contextual resolution.
    #[must_use]
    pub const fn port1(&self) -> &SourcedValue<NetworkEndpoint> {
        &self.port1
    }

    /// Borrows the second original endpoint's contextual resolution.
    #[must_use]
    pub const fn port2(&self) -> &SourcedValue<NetworkEndpoint> {
        &self.port2
    }
}

impl RoomAssociationBinding {
    /// Borrows the physical context that declares the room association.
    #[must_use]
    pub const fn context(&self) -> &LoadoutPosition {
        &self.context
    }

    /// Borrows the original component that declares the association.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the ordinal of the original associated container.
    #[must_use]
    pub const fn container_ordinal(&self) -> &SourcedValue<u32> {
        &self.container_ordinal
    }

    /// Borrows the original selector's contextual resolution.
    #[must_use]
    pub const fn port(&self) -> &SourcedValue<NetworkEndpoint> {
        &self.port
    }
}
