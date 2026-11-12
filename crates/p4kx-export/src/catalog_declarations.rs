// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use super::blueprint_category_list::BlueprintCategoryList;
use super::catalog_ammo_bindings::AmmoBinding;
use super::catalog_table::CatalogTable;
use super::resource_network_links::ResourceNetworkLinks;
use super::room_associations::RoomAssociations;
use super::tag_catalog::{EntityTags, TagRoots};
use crate::schema_fields::every_field_required;
use crate::shop_inventory_resolution::ShopInventoryResolution;

/// Original ordered uses, independently of reusable definitions and derived paths.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogDeclarations<'source> {
    pub(crate) blueprint_category_lists: CatalogTable<BlueprintCategoryList<'source>>,
    pub(crate) tag_roots: CatalogTable<TagRoots<'source>>,
    pub(crate) shop_inventory_resolution: CatalogTable<ShopInventoryResolution>,
    pub(crate) entity_tags: CatalogTable<EntityTags<'source>>,
    pub(crate) ammo_bindings: CatalogTable<AmmoBinding<'source>>,
    pub(crate) contract_scenario_requirements: CatalogTable<
        super::mission_sources::contract_scenario_requirements::ContractScenarioRequirements<
            'source,
        >,
    >,
    pub(crate) resource_network_links: CatalogTable<ResourceNetworkLinks<'source>>,
    pub(crate) room_associations: CatalogTable<RoomAssociations<'source>>,
    pub(crate) resource_group_members:
        CatalogTable<super::resource_group_catalog::ResourceGroupMembers>,
}
