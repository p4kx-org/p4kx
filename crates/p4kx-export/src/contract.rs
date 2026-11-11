// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogProvenance;
use crate::catalog_definitions::CatalogDefinitions;
use crate::catalog_projections::CatalogProjections;
use crate::catalog_text_contract::CatalogTexts;

pub(crate) use crate::catalog_v3::resource_group_catalog::ResourceGroup;
pub(crate) use crate::commodity_contract::Commodity;
pub(crate) use crate::contract_cargo::CargoManifest;
pub(crate) use crate::contract_completeness::{Completeness, KindCompleteness};
pub(crate) use crate::contract_crafter::Crafter;
pub(crate) use crate::contract_entities::{Item, Paint, Ship};
pub(crate) use crate::contract_harvest::{HarvestableLocation, HarvestableProviderPreset};
pub(crate) use crate::contract_harvest_config::SubHarvestableMultiConfig;
pub(crate) use crate::contract_loadout::Loadout;
pub(crate) use crate::contract_loot::{LootArchetype, LootContainer, LootTable};
pub(crate) use crate::contract_loot_legacy::{LegacyLootArchetype, LegacyLootTable};
pub(crate) use crate::contract_mining::{
    CompositionPart, MineableComposition, MineableElement, MineableEntity, RefiningProcess,
};
pub(crate) use crate::contract_mining_params::MiningGlobalParams;
pub(crate) use crate::contract_mission_data::{ContractScenario, MissionLocality, MissionType};
pub(crate) use crate::contract_missions::ContractEntry;
pub(crate) use crate::contract_reputation::{
    FactionReputation, ReputationContext, ReputationScope, ReputationStanding,
};
pub(crate) use crate::contract_rewards::{BlueprintPool, CraftingGlobal, DismantleProcess};
pub(crate) use crate::contract_shops::Shop;
pub(crate) use crate::contract_source::{BuildSource, Source};
pub(crate) use crate::contract_starmap::StarmapObject;
pub(crate) use crate::contract_store_slugs::ShipStoreSlug;
pub(crate) use crate::entity_spawn_model::EntitySpawnPool;
pub(crate) use crate::location_template::MissionLocationTemplate;
pub(crate) use crate::mission_property_set::MissionPropertySet;
pub(crate) use crate::npc_spawn_model::NpcSpawnPool;

pub(crate) use crate::resource_type_contract::ResourceType;
pub(crate) use crate::schema_fields::every_field_required;
pub(crate) use crate::spawn_model::{LocationQuery, ShipSpawnPool};

/// The version of the catalog shape and its closed payload unions.
pub const SCHEMA_VERSION: u32 = 3;

/// The Basetool catalog: the game data the basetool's P4K import reads.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[schemars(title = "P4KX Basetool catalog")]
pub(crate) struct Catalog<'a> {
    /// The version of the catalog shape; an import rejects a version it does not know.
    #[schemars(extend("const" = SCHEMA_VERSION))]
    pub(crate) schema_version: u32,
    /// The Archive and Build the catalog was made from.
    pub(crate) source: Source<'a>,
    /// Whether each kind is a complete snapshot of the Build.
    pub(crate) completeness: Completeness<'a>,
    /// The measured source-read and projection health, separately from Filter selection scope.
    pub(crate) export_health: crate::ExportHealth,
    /// The items, ordered by GUID.
    pub(crate) items: Vec<Item<'a>>,
    /// The ships, ordered by GUID.
    pub(crate) ships: Vec<Ship<'a>>,
    /// Canonical source-owned definition rows.
    pub(crate) definitions: CatalogDefinitions<'a>,
    /// Contextual configurations navigating canonical definitions.
    pub(crate) configurations: crate::catalog_configurations::CatalogConfigurations<'a>,
    /// Ordered authored declarations from selected physical Records.
    pub(crate) declarations: crate::catalog_v3::catalog_declarations::CatalogDeclarations<'a>,
    /// Explicit derived choices using the canonical definitions' source facts.
    pub(crate) projections: CatalogProjections<'a>,
    /// Physical source observations and actual contribution uses.
    pub(crate) provenance: CatalogProvenance,
    /// Public short-name and protected game-text tables.
    pub(crate) texts: CatalogTexts<'a>,
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
    /// The distinct NPC spawn descriptions of the contracts, templates and broker entries, ordered
    /// by identifier.
    pub(crate) contract_npc_spawns: Vec<NpcSpawnPool<'a>>,
    /// The distinct entity spawn descriptions of the contracts, templates and broker entries,
    /// ordered by identifier.
    pub(crate) contract_entity_spawns: Vec<EntitySpawnPool>,
    /// Canonical authored mission-property sets, ordered by stable source identifier.
    pub(crate) mission_property_sets: Vec<MissionPropertySet<'a>>,
    /// The RSI store slugs that map to a ship of `ships`, with the retired ones flagged, ordered
    /// by `slug`. A curated list, not read from the Archive.
    pub(crate) ship_store_slugs: Vec<ShipStoreSlug<'a>>,
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

/// Returns the JSON Schema (draft 2020-12) of the Basetool catalog.
#[must_use]
pub fn catalog_schema() -> serde_json::Value {
    schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<Catalog<'static>>()
        .to_value()
}
