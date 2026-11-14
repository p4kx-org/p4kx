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
use crate::catalog_v3::faction_identity_definition::FactionIdentityDefinition;
use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyDefinition;
use crate::catalog_v3::inventory_container_definition::InventoryContainerDefinition;
use crate::catalog_v3::law_kinds::LawTables;
use crate::catalog_v3::location_vocabulary::LocationVocabularyTables;
use crate::catalog_v3::loot_definition_catalog::LootDefinitionTables;
use crate::catalog_v3::medical::medical_tier_config::MedicalItemTierConfig;
use crate::catalog_v3::mission_sources::module_declaration::MissionModuleDeclaration;
use crate::catalog_v3::npc_spawn_definitions::NpcSpawnDefinitionsTables;
use crate::catalog_v3::reputation_context_definition::ContextDefinition;
use crate::catalog_v3::reputation_faction_definition::FactionReputationDefinition;
use crate::catalog_v3::reputation_perk_list::ReputationPerkList;
use crate::catalog_v3::reputation_reward_amount_definition::ReputationRewardAmountDefinition;
use crate::catalog_v3::reputation_scope_definition::ScopeDefinition;
use crate::catalog_v3::reputation_standing_definition::StandingDefinition;
use crate::catalog_v3::sandbox_trigger_record_definition::SandboxTriggerRecordDefinition;
use crate::catalog_v3::starmap_definition_kinds::StarmapDefinitionTables;
use crate::catalog_v3::weapon_action_recoil_definitions::WeaponActionRecoilDefinition;
use crate::catalog_v3::weapon_procedural_definitions::WeaponProceduralTables;
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
    pub(crate) mission_module_declarations: CatalogTable<MissionModuleDeclaration<'source>>,
    pub(crate) mission_items: CatalogTable<
        crate::catalog_v3::mission_sources::item_definition::MissionItemDefinition<'source>,
    >,
    pub(crate) mission_scenarios:
        CatalogTable<super::mission_scenario_definition::MissionScenarioDefinition<'source>>,
    pub(crate) contract_templates: CatalogTable<TemplateDefinition<'source>>,
    pub(crate) mission_broker_entries: CatalogTable<BrokerDefinition<'source>>,
    #[serde(flatten)]
    pub(crate) loot: LootDefinitionTables<'source>,
    pub(crate) blueprint_categories: CatalogTable<BlueprintCategoryDefinition<'source>>,
    pub(crate) gameplay_properties: CatalogTable<GameplayPropertyDefinition<'source>>,
    pub(crate) quality_distributions: CatalogTable<QualityDistributionDefinition<'source>>,
    pub(crate) quality_quantizations: CatalogTable<QualityQuantizationDefinition<'source>>,
    pub(crate) tags: CatalogTable<super::tag_catalog::TagDefinition<'source>>,
    pub(crate) shop_inventories: ShopInventoryTable,
    pub(crate) explosion_definitions: CatalogTable<ExplosionDefinition<'source>>,
    pub(crate) inventory_containers: CatalogTable<InventoryContainerDefinition<'source>>,
    pub(crate) medical_item_tier_configs: CatalogTable<MedicalItemTierConfig<'source>>,
    pub(crate) ammo_definitions: CatalogTable<AmmoDefinition<'source>>,
    pub(crate) weapon_action_recoil_definitions:
        CatalogTable<WeaponActionRecoilDefinition<'source>>,
    #[serde(flatten)]
    pub(crate) weapon_procedural: WeaponProceduralTables<'source>,
    pub(crate) reputation_contexts: CatalogTable<ContextDefinition<'source>>,
    pub(crate) reputation_scopes: CatalogTable<ScopeDefinition<'source>>,
    pub(crate) reputation_standings: CatalogTable<StandingDefinition<'source>>,
    pub(crate) resource_groups:
        CatalogTable<super::resource_group_catalog::ResourceGroupDefinition<'source>>,
    pub(crate) resource_types: CatalogTable<ResourceType<'source>>,
    pub(crate) commodities: CatalogTable<CommodityDefinition<'source>>,
    pub(crate) manufacturers: CatalogTable<ManufacturerDefinition<'source>>,
    pub(crate) faction_identities: CatalogTable<FactionIdentityDefinition<'source>>,
    pub(crate) factions: CatalogTable<FactionDefinition<'source>>,
    pub(crate) faction_reputations: CatalogTable<FactionReputationDefinition<'source>>,
    pub(crate) reputation_perks: CatalogTable<ReputationPerkList<'source>>,
    pub(crate) reputation_trigger_records: CatalogTable<SandboxTriggerRecordDefinition<'source>>,
    pub(crate) reputation_reward_amounts: CatalogTable<ReputationRewardAmountDefinition<'source>>,
    #[serde(flatten)]
    pub(crate) law: LawTables<'source>,
    #[serde(flatten)]
    pub(crate) location_vocabulary: LocationVocabularyTables<'source>,
    #[serde(flatten)]
    pub(crate) npc_spawn_definitions: NpcSpawnDefinitionsTables<'source>,
    #[serde(flatten)]
    pub(crate) starmap: StarmapDefinitionTables<'source>,
}
