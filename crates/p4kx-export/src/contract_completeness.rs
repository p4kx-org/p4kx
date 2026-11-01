// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;

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
    /// The completeness of `contractNpcSpawns`.
    pub(crate) contract_npc_spawns: KindCompleteness<'a>,
    /// The completeness of `contractEntitySpawns`.
    pub(crate) contract_entity_spawns: KindCompleteness<'a>,
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
            contract_npc_spawns: completeness,
            contract_entity_spawns: completeness,
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
