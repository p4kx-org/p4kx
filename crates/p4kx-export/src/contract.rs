// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use schemars::{JsonSchema, Schema};
use serde::Serialize;

use crate::guid_text::CatalogGuid;

pub(crate) use crate::broker_entry::BrokerEntry;
pub(crate) use crate::contract_cargo::CargoManifest;
pub(crate) use crate::contract_crafter::Crafter;
pub(crate) use crate::contract_crafting::{
    Blueprint, BlueprintCategory, Commodity, Ingredient, ResourceGroup,
};
pub(crate) use crate::contract_entities::{Item, Paint, Ship};
pub(crate) use crate::contract_harvest::{HarvestableLocation, HarvestableProviderPreset};
pub(crate) use crate::contract_harvest_config::SubHarvestableMultiConfig;
pub(crate) use crate::contract_loadout::Loadout;
pub(crate) use crate::contract_loot::{LootArchetype, LootContainer, LootPoolFilter, LootTable};
pub(crate) use crate::contract_loot_legacy::{LegacyLootArchetype, LegacyLootTable};
pub(crate) use crate::contract_mining::{
    CompositionPart, MineableComposition, MineableElement, MineableEntity, RefiningProcess,
};
pub(crate) use crate::contract_mining_params::MiningGlobalParams;
pub(crate) use crate::contract_mission_data::{ContractScenario, MissionLocality, MissionType};
pub(crate) use crate::contract_missions::ContractEntry;
pub(crate) use crate::contract_quantization::QualityQuantization;
pub(crate) use crate::contract_recovery::ItemRecovery;
pub(crate) use crate::contract_reputation::{
    FactionReputation, ReputationContext, ReputationScope, ReputationStanding,
};
pub(crate) use crate::contract_rewards::{
    BlueprintPool, CraftingGlobal, DismantleProcess, GameplayProperty, QualityDistribution,
};
pub(crate) use crate::contract_shops::Shop;
pub(crate) use crate::contract_source::{BuildSource, Source};
pub(crate) use crate::contract_starmap::StarmapObject;
pub(crate) use crate::contract_store_slugs::ShipStoreSlug;
pub(crate) use crate::contract_template::ContractTemplate;
pub(crate) use crate::location_template::MissionLocationTemplate;
pub(crate) use crate::spawn_model::{LocationQuery, ShipSpawnPool};

/// The version of the catalog shape.
///
/// Added kinds and fields keep it; renaming, removing or retyping a field, or restructuring a kind,
/// raises it (ADR-0054). Version 2 changes which ships a paint fits (ADR-0161) and has the
/// `qualityQuantizations` kind (ADR-0177).
pub const SCHEMA_VERSION: u32 = 2;

/// The Basetool catalog: the game data the basetool's P4K import reads.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[schemars(title = "P4KX Basetool catalog")]
pub(crate) struct Catalog<'a> {
    /// The version of the catalog shape; an import rejects a version it does not know.
    #[schemars(extend("const" = 2))]
    pub(crate) schema_version: u32,
    /// The Archive and Build the catalog was made from.
    pub(crate) source: Source<'a>,
    /// Whether each kind is a complete snapshot of the Build.
    pub(crate) completeness: Completeness<'a>,
    /// The manufacturers, ordered by GUID.
    pub(crate) manufacturers: Vec<Manufacturer<'a>>,
    /// The items, ordered by GUID.
    pub(crate) items: Vec<Item<'a>>,
    /// The ships, ordered by GUID.
    pub(crate) ships: Vec<Ship<'a>>,
    /// The commodities, ordered by GUID.
    pub(crate) commodities: Vec<Commodity<'a>>,
    /// The blueprints, ordered by GUID.
    pub(crate) blueprints: Vec<Blueprint<'a>>,
    /// The resource groups, ordered by GUID.
    pub(crate) resource_groups: Vec<ResourceGroup<'a>>,
    /// The blueprint categories, ordered by GUID.
    pub(crate) blueprint_categories: Vec<BlueprintCategory<'a>>,
    /// The gameplay property definitions the quality modifiers refer to, ordered by GUID.
    pub(crate) gameplay_properties: Vec<GameplayProperty<'a>>,
    /// The quality distributions and their location overrides, ordered by GUID.
    pub(crate) quality_distributions: Vec<QualityDistribution<'a>>,
    /// The quality quantizations, the quantised quality steps per `ResourceType`, ordered by GUID.
    pub(crate) quality_quantizations: Vec<QualityQuantization<'a>>,
    /// The blueprint pools with the places that reward them, ordered by GUID.
    pub(crate) blueprint_pools: Vec<BlueprintPool<'a>>,
    /// The dismantle processes, ordered by GUID.
    pub(crate) dismantle_processes: Vec<DismantleProcess<'a>>,
    /// The global crafting parameters that apply to every dismantle.
    pub(crate) crafting_global: CraftingGlobal,
    /// The paints with the ships they fit, ordered by the GUID of the item.
    pub(crate) paints: Vec<Paint>,
    /// The ports of every ship and of every item that has ports, with what the Default Loadout
    /// attaches to each, ordered by the GUID of the entity.
    pub(crate) loadouts: Vec<Loadout>,
    /// The starmap objects with their world positions, ordered by GUID.
    pub(crate) starmap_objects: Vec<StarmapObject<'a>>,
    /// The contracts and sub contracts with resolved titles, mission types and the reputation they
    /// grant on success, ordered by the GUID of the contract and then of the sub contract.
    pub(crate) contracts: Vec<ContractEntry<'a>>,
    /// The mineable elements, ordered by GUID. Several may link to one `ResourceType`.
    pub(crate) mineable_elements: Vec<MineableElement<'a>>,
    /// The refining processes, ordered by GUID.
    pub(crate) refining_processes: Vec<RefiningProcess<'a>>,
    /// The mineable compositions, the elements a rock or deposit is made of, ordered by GUID.
    pub(crate) mineable_compositions: Vec<MineableComposition<'a>>,
    /// The mining parameters of each kind of miner, ordered by GUID.
    pub(crate) mining_global_params: Vec<MiningGlobalParams<'a>>,
    /// The harvestable provider presets, what a place can spawn, ordered by GUID.
    pub(crate) harvestable_provider_presets: Vec<HarvestableProviderPreset<'a>>,
    /// The sub harvestable multi configs, what the slots of an object can hold, ordered by GUID.
    pub(crate) sub_harvestable_multi_configs: Vec<SubHarvestableMultiConfig<'a>>,
    /// The containers whose `.soc` names a provider preset, with the starmap object they hang
    /// below, ordered by preset GUID, container, system and object GUID.
    pub(crate) harvestable_locations: Vec<HarvestableLocation>,
    /// The containers placed with a multi config, with the starmap object each hangs below,
    /// ordered by config, container, system and object.
    pub(crate) loot_containers: Vec<LootContainer>,
    /// The shops of the shop registry with kind, brand and place, ordered by `key`.
    pub(crate) shops: Vec<Shop>,
    /// The factions with their reputation links, ordered by GUID.
    pub(crate) faction_reputations: Vec<FactionReputation<'a>>,
    /// The reputation scopes with the standings of each, ordered by GUID.
    pub(crate) reputation_scopes: Vec<ReputationScope<'a>>,
    /// The reputation standings, the ranks of the scopes, ordered by GUID.
    pub(crate) reputation_standings: Vec<ReputationStanding<'a>>,
    /// The reputation contexts, the scopes a faction is shown with, ordered by GUID.
    pub(crate) reputation_contexts: Vec<ReputationContext<'a>>,
    /// The mission types, the targets of `contracts[].missionTypeGuid`, ordered by GUID.
    pub(crate) mission_types: Vec<MissionType<'a>>,
    /// The mission localities, sets of starmap objects, ordered by GUID.
    pub(crate) mission_localities: Vec<MissionLocality<'a>>,
    /// The contract scenarios with their reward tiers, ordered by GUID.
    pub(crate) contract_scenarios: Vec<ContractScenario<'a>>,
    /// The loot tables with the weighted archetypes of each, ordered by GUID.
    pub(crate) loot_tables: Vec<LootTable<'a>>,
    /// The loot archetypes with the weighted selectors of each, ordered by GUID.
    pub(crate) loot_archetypes: Vec<LootArchetype<'a>>,
    /// The loot pool filters, ordered by GUID.
    pub(crate) loot_pool_filters: Vec<LootPoolFilter<'a>>,
    /// The older loot tables, the targets of `lootConfig.lootTableGuid`, ordered by GUID.
    pub(crate) legacy_loot_tables: Vec<LegacyLootTable<'a>>,
    /// The older loot archetypes the older tables draw from, ordered by GUID.
    pub(crate) legacy_loot_archetypes: Vec<LegacyLootArchetype<'a>>,
    /// The mineable entities with the scan signature of each, ordered by GUID.
    pub(crate) mineable_entities: Vec<MineableEntity<'a>>,
    /// The crafting stations with the limits of their job queues, ordered by GUID.
    pub(crate) crafters: Vec<Crafter<'a>>,
    /// The cargo manifests, how the game fills the cargo grid of a spawned ship, ordered by GUID.
    pub(crate) cargo_manifests: Vec<CargoManifest<'a>>,
    /// The mission location templates, the places a location query matches by tags, ordered by
    /// GUID.
    pub(crate) mission_location_templates: Vec<MissionLocationTemplate<'a>>,
    /// The distinct ship spawn descriptions of the contracts, ordered by identifier.
    pub(crate) contract_ship_spawns: Vec<ShipSpawnPool>,
    /// The distinct location queries of the contracts, ordered by identifier.
    pub(crate) contract_location_queries: Vec<LocationQuery>,
    /// The contract templates, the targets of `contracts[].templateGuid`, ordered by GUID.
    pub(crate) contract_templates: Vec<ContractTemplate<'a>>,
    /// The mission broker entries, the targets of `contracts[].brokerEntryGuid`, ordered by GUID.
    pub(crate) mission_broker_entries: Vec<BrokerEntry<'a>>,
    /// The RSI store slugs that map to a ship of `ships`, with the retired ones flagged, ordered
    /// by `slug`. A curated list, not read from the Archive.
    pub(crate) ship_store_slugs: Vec<ShipStoreSlug<'a>>,
    /// The item recovery rules, the multipliers the game applies to claims, `null` when the
    /// Archive has no such Record. Not a kind.
    pub(crate) item_recovery: Option<ItemRecovery<'a>>,
}

/// The completeness of every kind.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Completeness<'a> {
    /// The completeness of `manufacturers`.
    pub(crate) manufacturers: KindCompleteness<'a>,
    /// The completeness of `items`.
    pub(crate) items: KindCompleteness<'a>,
    /// The completeness of `ships`.
    pub(crate) ships: KindCompleteness<'a>,
    /// The completeness of `commodities`.
    pub(crate) commodities: KindCompleteness<'a>,
    /// The completeness of `blueprints`.
    pub(crate) blueprints: KindCompleteness<'a>,
    /// The completeness of `resourceGroups`.
    pub(crate) resource_groups: KindCompleteness<'a>,
    /// The completeness of `blueprintCategories`.
    pub(crate) blueprint_categories: KindCompleteness<'a>,
    /// The completeness of `gameplayProperties`.
    pub(crate) gameplay_properties: KindCompleteness<'a>,
    /// The completeness of `qualityDistributions`.
    pub(crate) quality_distributions: KindCompleteness<'a>,
    /// The completeness of `qualityQuantizations`.
    pub(crate) quality_quantizations: KindCompleteness<'a>,
    /// The completeness of `blueprintPools`.
    pub(crate) blueprint_pools: KindCompleteness<'a>,
    /// The completeness of `dismantleProcesses`.
    pub(crate) dismantle_processes: KindCompleteness<'a>,
    /// The completeness of `paints`.
    pub(crate) paints: KindCompleteness<'a>,
    /// The completeness of `loadouts`.
    pub(crate) loadouts: KindCompleteness<'a>,
    /// The completeness of `starmapObjects`.
    pub(crate) starmap_objects: KindCompleteness<'a>,
    /// The completeness of `contracts`.
    pub(crate) contracts: KindCompleteness<'a>,
    /// The completeness of `mineableElements`.
    pub(crate) mineable_elements: KindCompleteness<'a>,
    /// The completeness of `refiningProcesses`.
    pub(crate) refining_processes: KindCompleteness<'a>,
    /// The completeness of `mineableCompositions`.
    pub(crate) mineable_compositions: KindCompleteness<'a>,
    /// The completeness of `miningGlobalParams`.
    pub(crate) mining_global_params: KindCompleteness<'a>,
    /// The completeness of `harvestableProviderPresets`.
    pub(crate) harvestable_provider_presets: KindCompleteness<'a>,
    /// The completeness of `subHarvestableMultiConfigs`.
    pub(crate) sub_harvestable_multi_configs: KindCompleteness<'a>,
    /// The completeness of `harvestableLocations`.
    pub(crate) harvestable_locations: KindCompleteness<'a>,
    /// The completeness of `lootContainers`.
    pub(crate) loot_containers: KindCompleteness<'a>,
    /// The completeness of `shops`.
    pub(crate) shops: KindCompleteness<'a>,
    /// The completeness of `factionReputations`.
    pub(crate) faction_reputations: KindCompleteness<'a>,
    /// The completeness of `reputationScopes`.
    pub(crate) reputation_scopes: KindCompleteness<'a>,
    /// The completeness of `reputationStandings`.
    pub(crate) reputation_standings: KindCompleteness<'a>,
    /// The completeness of `reputationContexts`.
    pub(crate) reputation_contexts: KindCompleteness<'a>,
    /// The completeness of `missionTypes`.
    pub(crate) mission_types: KindCompleteness<'a>,
    /// The completeness of `missionLocalities`.
    pub(crate) mission_localities: KindCompleteness<'a>,
    /// The completeness of `contractScenarios`.
    pub(crate) contract_scenarios: KindCompleteness<'a>,
    /// The completeness of `lootTables`.
    pub(crate) loot_tables: KindCompleteness<'a>,
    /// The completeness of `lootArchetypes`.
    pub(crate) loot_archetypes: KindCompleteness<'a>,
    /// The completeness of `lootPoolFilters`.
    pub(crate) loot_pool_filters: KindCompleteness<'a>,
    /// The completeness of `legacyLootTables`.
    pub(crate) legacy_loot_tables: KindCompleteness<'a>,
    /// The completeness of `legacyLootArchetypes`.
    pub(crate) legacy_loot_archetypes: KindCompleteness<'a>,
    /// The completeness of `mineableEntities`.
    pub(crate) mineable_entities: KindCompleteness<'a>,
    /// The completeness of `crafters`.
    pub(crate) crafters: KindCompleteness<'a>,
    /// The completeness of `cargoManifests`.
    pub(crate) cargo_manifests: KindCompleteness<'a>,
    /// The completeness of `missionLocationTemplates`.
    pub(crate) mission_location_templates: KindCompleteness<'a>,
    /// The completeness of `contractShipSpawns`.
    pub(crate) contract_ship_spawns: KindCompleteness<'a>,
    /// The completeness of `contractLocationQueries`.
    pub(crate) contract_location_queries: KindCompleteness<'a>,
    /// The completeness of `contractTemplates`.
    pub(crate) contract_templates: KindCompleteness<'a>,
    /// The completeness of `missionBrokerEntries`.
    pub(crate) mission_broker_entries: KindCompleteness<'a>,
    /// The completeness of `shipStoreSlugs`.
    pub(crate) ship_store_slugs: KindCompleteness<'a>,
}

impl<'a> Completeness<'a> {
    /// Returns the completeness that states `completeness` for every kind.
    pub(crate) const fn of_every_kind(completeness: KindCompleteness<'a>) -> Self {
        Self {
            manufacturers: completeness,
            items: completeness,
            ships: completeness,
            commodities: completeness,
            blueprints: completeness,
            resource_groups: completeness,
            blueprint_categories: completeness,
            gameplay_properties: completeness,
            quality_distributions: completeness,
            quality_quantizations: completeness,
            blueprint_pools: completeness,
            dismantle_processes: completeness,
            paints: completeness,
            loadouts: completeness,
            starmap_objects: completeness,
            contracts: completeness,
            mineable_elements: completeness,
            refining_processes: completeness,
            mineable_compositions: completeness,
            mining_global_params: completeness,
            harvestable_provider_presets: completeness,
            sub_harvestable_multi_configs: completeness,
            harvestable_locations: completeness,
            loot_containers: completeness,
            shops: completeness,
            faction_reputations: completeness,
            reputation_scopes: completeness,
            reputation_standings: completeness,
            reputation_contexts: completeness,
            mission_types: completeness,
            mission_localities: completeness,
            contract_scenarios: completeness,
            loot_tables: completeness,
            loot_archetypes: completeness,
            loot_pool_filters: completeness,
            legacy_loot_tables: completeness,
            legacy_loot_archetypes: completeness,
            mineable_entities: completeness,
            crafters: completeness,
            cargo_manifests: completeness,
            mission_location_templates: completeness,
            contract_ship_spawns: completeness,
            contract_location_queries: completeness,
            contract_templates: completeness,
            mission_broker_entries: completeness,
            ship_store_slugs: completeness,
        }
    }
}

/// Whether a kind holds every Record of the Build or only those a Filter selected.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum KindCompleteness<'a> {
    /// Every Record of the kind is listed; one missing from the catalog was removed from the game.
    Complete,
    /// Only the Records the Filter selected are listed.
    Partial {
        /// The Filter text.
        filter: &'a str,
    },
}

/// A manufacturer: an `SCItemManufacturer` Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Manufacturer<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `ACAS` for `SCItemManufacturer.ACAS`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The manufacturer code, e.g. `ACAS`, `null` when it cannot be read.
    pub(crate) code: Option<&'a str>,
    /// The name.
    pub(crate) name: LocalisedText<'a>,
    /// The description.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: LocalisedText<'a>,
}

/// A text the game localises: its localisation key and the resolved English and German texts,
/// as the language's `global.ini` stores them with the escape `\n` decoded to a line feed.
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocalisedText<'a> {
    /// The localisation key, e.g. `@manufacturer_NameACAS`, `null` when it is unknown.
    pub(crate) key: Option<&'a str>,
    /// The English text, `null` for no key, a placeholder key, or a key the English `global.ini`
    /// lacks or holds empty.
    pub(crate) en: Option<Cow<'a, str>>,
    /// The German text, `null` for no key, a placeholder key, or a key the German `global.ini`
    /// lacks or holds empty.
    pub(crate) de: Option<Cow<'a, str>>,
}

/// Makes every property of an object schema required: the catalog writes `null`, never omits.
pub(crate) fn every_field_required(schema: &mut Schema) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    let names: Vec<serde_json::Value> = object
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .map(|properties| {
            properties
                .keys()
                .cloned()
                .map(serde_json::Value::String)
                .collect()
        })
        .unwrap_or_default();
    object.insert("required".to_owned(), serde_json::Value::Array(names));
}

/// Returns the JSON Schema (draft 2020-12) of the Basetool catalog.
#[must_use]
pub fn catalog_schema() -> serde_json::Value {
    schemars::schema_for!(Catalog<'static>).to_value()
}
