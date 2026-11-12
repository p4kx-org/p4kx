// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::blueprint_category::BlueprintCategoryDefinition;
use crate::broker_definition::BrokerDefinition;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::ammo_definition::AmmoDefinition;
use crate::catalog_v3::explosion_definition::ExplosionDefinition;
use crate::catalog_v3::faction_definition::FactionDefinition;
use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyDefinition;
use crate::catalog_v3::loot_filter_catalog::LootFilterDefinition;
use crate::catalog_v3::reputation_scope_definition::ScopeDefinition;
use crate::catalog_v3::reputation_standing_definition::StandingDefinition;
use crate::commodity_contract::CommodityDefinition;
use crate::manufacturer::ManufacturerDefinition;
use crate::mission_generator::GeneratorDefinition;
use crate::quality_catalog::contract::{
    QualityDistributionDefinition, QualityQuantizationDefinition,
};
use crate::resource_type_contract::ResourceType;
use crate::schema_fields::every_field_required;
use crate::shop_inventories::ShopInventoryTable;
use crate::template_definition::TemplateDefinition;

/// Canonical typed definitions with distinct physical identities and source scopes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogDefinitions<'source> {
    pub(crate) blueprints: CatalogTable<crate::blueprint_definition::BlueprintDefinition<'source>>,
    pub(crate) contract_generators: CatalogTable<GeneratorDefinition<'source>>,
    pub(crate) contract_templates: CatalogTable<TemplateDefinition<'source>>,
    pub(crate) mission_broker_entries: CatalogTable<BrokerDefinition<'source>>,
    pub(crate) loot_pool_filters: CatalogTable<LootFilterDefinition<'source>>,
    pub(crate) blueprint_categories: CatalogTable<BlueprintCategoryDefinition<'source>>,
    pub(crate) gameplay_properties: CatalogTable<GameplayPropertyDefinition<'source>>,
    pub(crate) quality_distributions: CatalogTable<QualityDistributionDefinition<'source>>,
    pub(crate) quality_quantizations: CatalogTable<QualityQuantizationDefinition<'source>>,
    pub(crate) tags: CatalogTable<super::tag_catalog::TagDefinition<'source>>,
    pub(crate) shop_inventories: ShopInventoryTable,
    pub(crate) explosion_definitions: CatalogTable<ExplosionDefinition<'source>>,
    pub(crate) ammo_definitions: CatalogTable<AmmoDefinition<'source>>,
    pub(crate) reputation_scopes: CatalogTable<ScopeDefinition<'source>>,
    pub(crate) reputation_standings: CatalogTable<StandingDefinition<'source>>,
    pub(crate) resource_groups:
        CatalogTable<super::resource_group_catalog::ResourceGroupDefinition<'source>>,
    pub(crate) resource_types: CatalogTable<ResourceType<'source>>,
    pub(crate) commodities: CatalogTable<CommodityDefinition<'source>>,
    pub(crate) manufacturers: CatalogTable<ManufacturerDefinition<'source>>,
    pub(crate) factions: CatalogTable<FactionDefinition<'source>>,
}
