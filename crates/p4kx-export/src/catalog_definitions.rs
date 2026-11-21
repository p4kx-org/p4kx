// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::blueprint_category::BlueprintCategoryDefinition;
use crate::broker_definition::BrokerDefinition;
use crate::build_derived::particle_library::ParticleLibrary;
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::ammo_definition::AmmoDefinition;
use crate::catalog_v3::cargo_manifest::CargoManifestDefinition;
use crate::catalog_v3::catalog_independent_configurations::{
    ConsumableDefinitions, DamageResistanceMacro, MiningLaserGlobalDefinition, TurretHealthModifier,
};
use crate::catalog_v3::catalog_resource_network_read::resource_network_global::ResourceNetworkGlobal;
use crate::catalog_v3::explosion_definition::ExplosionDefinition;
use crate::catalog_v3::faction_definition::FactionDefinition;
use crate::catalog_v3::faction_identity_definition::FactionIdentityDefinition;
use crate::catalog_v3::flare_library::FlareLibrary;
use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyDefinition;
use crate::catalog_v3::industry_modules::dismantle_process_definition::DismantleProcessDefinition;
use crate::catalog_v3::inventory_container_definition::InventoryContainerDefinition;
use crate::catalog_v3::law_kinds::LawTables;
use crate::catalog_v3::location_vocabulary::LocationVocabularyTables;
use crate::catalog_v3::loot_definition_catalog::LootDefinitionTables;
use crate::catalog_v3::medical::medical_tier_config::MedicalItemTierConfig;
use crate::catalog_v3::mission_sources::module_declaration::MissionModuleDeclaration;
use crate::catalog_v3::npc_spawn_definitions::NpcSpawnDefinitionsTables;
use crate::catalog_v3::procedural_planet_audio::ProceduralPlanetAudio;
use crate::catalog_v3::reputation_context_definition::ContextDefinition;
use crate::catalog_v3::reputation_faction_definition::FactionReputationDefinition;
use crate::catalog_v3::reputation_perk_list::ReputationPerkList;
use crate::catalog_v3::reputation_reward_amount_definition::ReputationRewardAmountDefinition;
use crate::catalog_v3::reputation_scope_definition::ScopeDefinition;
use crate::catalog_v3::reputation_standing_definition::StandingDefinition;
use crate::catalog_v3::sandbox_trigger_record_definition::SandboxTriggerRecordDefinition;
use crate::catalog_v3::starmap_definition_kinds::StarmapDefinitionTables;
use crate::catalog_v3::subsumption_event::SubsumptionEventDocument;
use crate::catalog_v3::weapon_action_recoil_definitions::WeaponActionRecoilDefinition;
use crate::catalog_v3::weapon_procedural_definitions::WeaponProceduralTables;
use crate::commodity_contract::CommodityDefinition;
use crate::contract::MineableEntity;
use crate::manufacturer::ManufacturerDefinition;
use crate::mission_generator::GeneratorDefinition;
use planet_tech_definitions::PlanetTechDefinitions;

#[path = "ecosystem.rs"]
pub(crate) mod ecosystem;
#[path = "geology_set.rs"]
mod geology_set;
#[path = "geology_texture_set.rs"]
mod geology_texture_set;
#[path = "planet_asset_raw_fields.rs"]
pub(crate) mod planet_asset_raw_fields;
#[cfg(test)]
#[path = "planet_asset_tests.rs"]
mod planet_asset_tests;
#[path = "planet_climate_asset.rs"]
mod planet_climate_asset;
#[path = "planet_climate_properties.rs"]
mod planet_climate_properties;
#[path = "planet_climate_subasset.rs"]
mod planet_climate_subasset;
#[path = "planet_creature.rs"]
pub(crate) mod planet_creature;
#[path = "planet_geology_asset.rs"]
mod planet_geology_asset;
#[path = "planet_geology_properties.rs"]
mod planet_geology_properties;
#[path = "planet_geology_references.rs"]
mod planet_geology_references;
#[path = "planet_palette_reference.rs"]
mod planet_palette_reference;
#[path = "planet_resource.rs"]
pub(crate) mod planet_resource;
#[path = "planet_tech_definitions.rs"]
pub(crate) mod planet_tech_definitions;
#[path = "shading_class.rs"]
pub(crate) mod shading_class;
#[path = "soil.rs"]
pub(crate) mod soil;
#[path = "soil_texture_set.rs"]
pub(crate) mod soil_texture_set;
#[cfg(test)]
#[path = "terrain_definition_witness.rs"]
mod terrain_definition_witness;
#[path = "terrain_guid_link.rs"]
mod terrain_guid_link;
#[cfg(test)]
#[path = "terrain_guid_link_tests.rs"]
mod terrain_guid_link_tests;
#[path = "terrain_layers.rs"]
pub(crate) mod terrain_layers;
#[cfg(test)]
#[path = "terrain_projection_witness.rs"]
mod terrain_projection_witness;
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
    #[schemars(
        with = "CatalogTable<super::catalog_item_components::item_definition::ItemDefinition<'source>>"
    )]
    pub(crate) items: StagedCatalogTable,
    #[schemars(with = "CatalogTable<crate::blueprint_definition::BlueprintDefinition<'source>>")]
    pub(crate) blueprints: StagedCatalogTable,
    pub(crate) dismantle_processes: CatalogTable<DismantleProcessDefinition<'source>>,
    #[schemars(with = "CatalogTable<GeneratorDefinition<'source>>")]
    pub(crate) contract_generators: StagedCatalogTable,
    pub(crate) cargo_manifests: CatalogTable<CargoManifestDefinition<'source>>,
    pub(crate) mission_module_declarations: CatalogTable<MissionModuleDeclaration<'source>>,
    pub(crate) mission_items: CatalogTable<
        crate::catalog_v3::mission_sources::item_definition::MissionItemDefinition<'source>,
    >,
    pub(crate) mission_scenarios:
        CatalogTable<super::mission_scenario_definition::MissionScenarioDefinition<'source>>,
    pub(crate) mission_types: CatalogTable<crate::contract_mission_data::MissionType<'source>>,
    pub(crate) mission_localities: CatalogTable<
        crate::catalog_v3::mission_locality_definition::MissionLocalityDefinition<'source>,
    >,
    #[schemars(with = "CatalogTable<TemplateDefinition<'source>>")]
    pub(crate) contract_templates: StagedCatalogTable,
    #[schemars(with = "CatalogTable<BrokerDefinition<'source>>")]
    pub(crate) mission_broker_entries: StagedCatalogTable,
    #[serde(flatten)]
    pub(crate) loot: LootDefinitionTables<'source>,
    pub(crate) blueprint_pools: CatalogTable<
        crate::catalog_v3::industry_modules::blueprint_pool_definition::BlueprintPoolDefinition<
            'source,
        >,
    >,
    pub(crate) blueprint_categories: CatalogTable<BlueprintCategoryDefinition<'source>>,
    pub(crate) gameplay_properties: CatalogTable<GameplayPropertyDefinition<'source>>,
    pub(crate) mineable_entities: CatalogTable<MineableEntity<'source>>,
    pub(crate) refining_processes:
        CatalogTable<crate::catalog_v3::mining_process_catalog::RefiningProcessDefinition<'source>>,
    pub(crate) quality_distributions: CatalogTable<QualityDistributionDefinition<'source>>,
    pub(crate) quality_quantizations: CatalogTable<QualityQuantizationDefinition<'source>>,
    pub(crate) tags: CatalogTable<super::tag_catalog::TagDefinition<'source>>,
    pub(crate) shop_inventories: ShopInventoryTable,
    #[schemars(with = "CatalogTable<ParticleLibrary>")]
    pub(crate) particle_libraries: StagedCatalogTable,
    #[schemars(with = "CatalogTable<FlareLibrary>")]
    pub(crate) flare_libraries: StagedCatalogTable,
    #[schemars(with = "CatalogTable<SubsumptionEventDocument>")]
    pub(crate) subsumption_events: StagedCatalogTable,
    #[serde(flatten)]
    pub(crate) planet_tech: PlanetTechDefinitions,
    pub(crate) explosion_definitions: CatalogTable<ExplosionDefinition<'source>>,
    pub(crate) inventory_containers: CatalogTable<InventoryContainerDefinition<'source>>,
    pub(crate) medical_item_tier_configs: CatalogTable<MedicalItemTierConfig<'source>>,
    pub(crate) procedural_planet_audio: CatalogTable<ProceduralPlanetAudio<'source>>,
    pub(crate) resource_network_globals: CatalogTable<ResourceNetworkGlobal<'source>>,
    pub(crate) ammo_definitions: CatalogTable<AmmoDefinition<'source>>,
    pub(crate) weapon_animation_sources:
        crate::catalog_v3::catalog_projections::weapon_reload::WeaponAnimationSources<'source>,
    pub(crate) weapon_action_recoil_definitions:
        CatalogTable<WeaponActionRecoilDefinition<'source>>,
    #[serde(flatten)]
    pub(crate) weapon_procedural: WeaponProceduralTables<'source>,
    pub(crate) damage_resistance_macros: CatalogTable<DamageResistanceMacro<'source>>,
    pub(crate) turret_health_modifiers: CatalogTable<TurretHealthModifier<'source>>,
    #[serde(flatten)]
    pub(crate) consumables: ConsumableDefinitions<'source>,
    pub(crate) mining_laser_globals: CatalogTable<MiningLaserGlobalDefinition<'source>>,
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
