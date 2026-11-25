// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogProvenance;
use crate::catalog_definitions::CatalogDefinitions;
use crate::catalog_projections::CatalogProjections;
use crate::catalog_text_contract::CatalogTexts;
use crate::loadout::staging::StagedLoadouts;

pub(crate) use crate::catalog_v3::catalog_harvest_modules::harvest_provider::HarvestableProviderPreset;
pub(crate) use crate::catalog_v3::mineable_composition_definition::MineableComposition;
pub(crate) use crate::catalog_v3::mineable_element_definition::MineableElement;
pub(crate) use crate::catalog_v3::mining_process_catalog::{
    PreparedMiningGlobal as MiningGlobalParams, PreparedRefining as RefiningProcess,
};
pub(crate) use crate::catalog_v3::mission_sources::scenario_progress::ContractScenario;
pub(crate) use crate::catalog_v3::resource_group_catalog::ResourceGroup;
pub(crate) use crate::commodity_contract::Commodity;
pub(crate) use crate::contract_completeness::{Completeness, KindCompleteness};
pub(crate) use crate::contract_entities::{Item, Paint, Ship};
pub(crate) use crate::contract_harvest_config::SubHarvestableMultiConfig;
pub(crate) use crate::contract_loadout::Loadout;
pub(crate) use crate::contract_loot::{LootArchetype, LootContainer, LootTable};
pub(crate) use crate::contract_loot_legacy::{LegacyLootArchetype, LegacyLootTable};
pub(crate) use crate::contract_mining::MineableEntity;
pub(crate) use crate::contract_missions::ContractEntry;
pub(crate) use crate::contract_shops::Shop;
pub(crate) use crate::contract_source::{BuildSource, Source};
pub(crate) use crate::contract_starmap::StarmapObject;
pub(crate) use crate::contract_store_slugs::ShipStoreSlug;
pub(crate) use crate::entity_spawn_model::EntitySpawnPool;
pub(crate) use crate::location_template::MissionLocationTemplate;
pub(crate) use crate::mining_locations::HarvestableLocation;
pub(crate) use crate::mission_property_set::MissionPropertySet;
pub(crate) use crate::npc_spawn_model::PublishedNpcPool;

pub(crate) use crate::resource_type_contract::ResourceType;
pub(crate) use crate::schema_fields::every_field_required;

#[path = "catalog_document.rs"]
pub(crate) mod document;
#[path = "catalog_marker_schema.rs"]
pub(crate) mod marker_schema;
pub(crate) use crate::spawn_model::{LocationQuery, ShipSpawnPool};

/// The version of the catalog shape and its closed payload unions.
pub const SCHEMA_VERSION: u32 = 3;

/// The versioned P4KX game-data catalog.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[schemars(title = "P4KX catalog")]
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
    /// Physical and derived placements linked to canonical definitions.
    pub(crate) placements: crate::catalog_v3::catalog_placements::CatalogPlacements,
    /// Explicit derived choices using the canonical definitions' source facts.
    pub(crate) projections: CatalogProjections<'a>,
    /// Physical source observations and actual contribution uses.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "CatalogProvenance")]
    pub(crate) provenance: Option<&'a CatalogProvenance>,
    /// Public short-name and protected game-text tables.
    pub(crate) texts: CatalogTexts<'a>,
    /// The paints with the ships they fit, ordered by the GUID of the item.
    pub(crate) paints: Vec<Paint>,
    /// The ports of every ship and of every item that has ports, with what the Default Loadout
    /// attaches to each, ordered by the GUID of the entity.
    #[schemars(with = "Vec<Loadout>")]
    pub(crate) loadouts: StagedLoadouts,

    /// The mineable elements, ordered by GUID. Several may link to one `ResourceType`.
    pub(crate) mineable_elements: Vec<MineableElement>,
    /// The mineable compositions, the elements a rock or deposit is made of, ordered by GUID.
    pub(crate) mineable_compositions: Vec<MineableComposition>,
    /// The harvestable provider presets, what a place can spawn, ordered by GUID.
    pub(crate) harvestable_provider_presets: Vec<HarvestableProviderPreset>,
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
    /// The mission location templates, the places a location query matches by tags, ordered by
    /// GUID.
    pub(crate) mission_location_templates: Vec<MissionLocationTemplate<'a>>,
    /// The distinct ship spawn descriptions of the contracts, ordered by identifier.
    pub(crate) contract_ship_spawns: Vec<ShipSpawnPool>,
    /// The distinct location queries of the contracts, ordered by identifier.
    pub(crate) contract_location_queries: Vec<LocationQuery>,
    /// The distinct NPC spawn descriptions of the contracts, templates and broker entries, ordered
    /// by identifier.
    pub(crate) contract_npc_spawns: Vec<PublishedNpcPool>,
    /// The distinct entity spawn descriptions of the contracts, templates and broker entries,
    /// ordered by identifier.
    pub(crate) contract_entity_spawns: Vec<EntitySpawnPool>,
    /// Canonical authored mission-property sets, ordered by stable source identifier.
    pub(crate) mission_property_sets: Vec<MissionPropertySet<'a>>,
    /// The RSI store slugs that map to a ship of `ships`, with the retired ones flagged, ordered
    /// by `slug`. A curated list, not read from the Archive.
    pub(crate) ship_store_slugs: Vec<ShipStoreSlug<'a>>,
    /// The non-flight-ready Store pledges admitted by the dated candidate review.
    pub(crate) ship_store_pledges: crate::ship_store_pledge::ShipStorePledges,
}

/// The serialized text-key shape in an NPC pool content identifier.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalisedText<'a> {
    /// The localisation key, e.g. `@manufacturer_NameACAS`, `null` when it is unknown.
    pub(crate) key: Option<&'a str>,
    /// The required null English field.
    pub(crate) en: Option<Cow<'a, str>>,
    /// The required null German field.
    pub(crate) de: Option<Cow<'a, str>>,
}

/// Returns the JSON Schema (draft 2020-12) of the P4KX catalog.
#[must_use]
pub fn catalog_schema() -> serde_json::Value {
    schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<Catalog<'static>>()
        .to_value()
}
