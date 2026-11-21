// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_item_components::source_getters;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceWeakReference};

pub(super) const PORT_CONTAINER: &str = "SItemPortContainerComponentParams";
pub(super) const PORT: &str = "SItemPortDef";
const HARDPOINT_LINK: &str = "SInternalHardpointLink";
const RESOURCE_LINK: &str = "SInternalResourceLink";
const HARDPOINT_LINKS: &str = "InternalHardpointLinks";
const RESOURCE_LINKS: &str = "InternalResourceLinks";
const LINK_ALL: &str = "LinkAllItemsToResourceNetwork";
const POWER_POOLS: &str = "resourceNetworkPowerPools";
pub(super) const MEMBERS: &[&str] = &[HARDPOINT_LINKS, RESOURCE_LINKS, LINK_ALL, POWER_POOLS];

/// The two independent ordered link families declared by a physical port container.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourceNetworkLinks<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    internal_hardpoint_links: SourcedValue<Vec<SourcedValue<HardpointLink<'source>>>>,
    internal_resource_links: SourcedValue<Vec<SourcedValue<ResourcePortLink<'source>>>>,
    link_all_items_to_resource_network: SourcedValue<bool>,
    resource_network_power_pools: SourcedValue<SourceArenaOutput<'source>>,
}

impl<'source> ResourceNetworkLinks<'source> {
    source_getters! {
        /// Borrows the original hardpoint link slots.
        internal_hardpoint_links: SourcedValue<Vec<SourcedValue<HardpointLink<'source>>>>,
        /// Borrows the original weak resource link slots.
        internal_resource_links: SourcedValue<Vec<SourcedValue<ResourcePortLink<'source>>>>,
        /// Borrows the authored link-all flag.
        link_all_items_to_resource_network: SourcedValue<bool>,
        /// Borrows the original power-pool declaration.
        resource_network_power_pools: SourcedValue<SourceArenaOutput<'source>>,
    }

    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Option<Self>, CatalogError> {
        let row = Self {
            owner,
            internal_hardpoint_links: source.class_array_to(
                HARDPOINT_LINKS,
                HARDPOINT_LINK,
                notes,
                HardpointLink::read,
            )?,
            internal_resource_links: source.class_array_to(
                RESOURCE_LINKS,
                RESOURCE_LINK,
                notes,
                ResourcePortLink::read,
            )?,
            link_all_items_to_resource_network: source.boolean(LINK_ALL, notes)?,
            resource_network_power_pools: source.declared_class_to(
                POWER_POOLS,
                "ResourceNetworkPowerPoolSetup",
                notes,
                |pools, _| pools.authored_members(),
            )?,
        };
        let declared = [
            row.internal_hardpoint_links.state(),
            row.internal_resource_links.state(),
            row.link_all_items_to_resource_network.state(),
            row.resource_network_power_pools.state(),
        ]
        .into_iter()
        .any(|state| state != SourceValueState::Absent);
        Ok(declared.then_some(row))
    }

    pub(super) fn hardpoints(&self) -> impl Iterator<Item = &HardpointLink<'source>> {
        self.internal_hardpoint_links
            .value()
            .into_iter()
            .flatten()
            .filter_map(SourcedValue::value)
    }

    pub(super) fn resources(&self) -> impl Iterator<Item = &ResourcePortLink<'source>> {
        self.internal_resource_links
            .value()
            .into_iter()
            .flatten()
            .filter_map(SourcedValue::value)
    }

    /// Borrows the original port-container component owner.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }
}

/// Two authored named hardpoints in one original link slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct HardpointLink<'source> {
    pub(super) link_ordinal: SourcedValue<u32>,
    #[serde(skip)]
    pub(super) port1_use: Option<SourceUse>,
    #[serde(skip)]
    pub(super) port2_use: Option<SourceUse>,
    pub(super) port1: SourcedValue<&'source str>,
    pub(super) port2: SourcedValue<&'source str>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> HardpointLink<'source> {
    source_getters! {
        /// Borrows the original array position.
        link_ordinal: SourcedValue<u32>,
        /// Borrows the first authored endpoint.
        port1: SourcedValue<&'source str>,
        /// Borrows the second authored endpoint.
        port2: SourcedValue<&'source str>,
        /// Borrows the remaining original members.
        additional_fields: SourceArenaOutput<'source>,
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let port1 = source.borrowed_string("port1", notes)?;
        let port2 = source.borrowed_string("port2", notes)?;
        let additional_fields =
            source.authored_members_except([("port1", port1.state()), ("port2", port2.state())])?;
        Ok(Self {
            link_ordinal: source.owned_element_ordinal()?,
            port1_use: source.selector_use("port1")?,
            port2_use: source.selector_use("port2")?,
            port1,
            port2,
            additional_fields,
        })
    }
}

/// Two authored weak port references in one original link slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourcePortLink<'source> {
    pub(super) link_ordinal: SourcedValue<u32>,
    #[serde(skip)]
    pub(super) port1_use: Option<SourceUse>,
    #[serde(skip)]
    pub(super) port2_use: Option<SourceUse>,
    pub(super) port1: SourcedValue<Box<SourceWeakReference<'source>>>,
    pub(super) port2: SourcedValue<Box<SourceWeakReference<'source>>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourcePortLink<'source> {
    source_getters! {
        /// Borrows the original array position.
        link_ordinal: SourcedValue<u32>,
        /// Borrows the first authored endpoint.
        port1: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// Borrows the second authored endpoint.
        port2: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// Borrows the remaining original members.
        additional_fields: SourceArenaOutput<'source>,
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let port1 = source.weak_link_to("port1", PORT, notes)?;
        let port2 = source.weak_link_to("port2", PORT, notes)?;
        let additional_fields =
            source.authored_members_except([("port1", port1.state()), ("port2", port2.state())])?;
        Ok(Self {
            link_ordinal: source.owned_element_ordinal()?,
            port1_use: source.selector_use("port1")?,
            port2_use: source.selector_use("port2")?,
            port1,
            port2,
            additional_fields,
        })
    }
}
