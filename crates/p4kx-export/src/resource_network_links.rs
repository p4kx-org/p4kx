// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceWeakReference};

pub(super) const PORT_CONTAINER: &str = "SItemPortContainerComponentParams";
pub(super) const PORT: &str = "SItemPortDef";
const HARDPOINT_LINK: &str = "SInternalHardpointLink";
const RESOURCE_LINK: &str = "SInternalResourceLink";

/// The two independent ordered link families declared by a physical port container.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResourceNetworkLinks<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    internal_hardpoint_links: SourcedValue<Vec<SourcedValue<HardpointLink<'source>>>>,
    internal_resource_links: SourcedValue<Vec<SourcedValue<ResourcePortLink<'source>>>>,
    link_all_items_to_resource_network: SourcedValue<bool>,
    resource_network_power_pools: SourcedValue<SourceArenaOutput<'source>>,
}

impl<'source> ResourceNetworkLinks<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Option<Self>, CatalogError> {
        let row = Self {
            owner,
            internal_hardpoint_links: source.class_array_to(
                "InternalHardpointLinks",
                HARDPOINT_LINK,
                notes,
                HardpointLink::read,
            )?,
            internal_resource_links: source.class_array_to(
                "InternalResourceLinks",
                RESOURCE_LINK,
                notes,
                ResourcePortLink::read,
            )?,
            link_all_items_to_resource_network: source
                .boolean("LinkAllItemsToResourceNetwork", notes)?,
            resource_network_power_pools: source.declared_class_to(
                "resourceNetworkPowerPools",
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

    pub(super) const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct HardpointLink<'source> {
    link_ordinal: SourcedValue<u32>,
    port1: SourcedValue<&'source str>,
    port2: SourcedValue<&'source str>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> HardpointLink<'source> {
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
            port1,
            port2,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ResourcePortLink<'source> {
    link_ordinal: SourcedValue<u32>,
    port1: SourcedValue<Box<SourceWeakReference<'source>>>,
    port2: SourcedValue<Box<SourceWeakReference<'source>>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourcePortLink<'source> {
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
            port1,
            port2,
            additional_fields,
        })
    }
}
