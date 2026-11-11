// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::Collected;
use crate::selection::Kind;

impl Collected<'_> {
    /// Returns the number of written rows for one catalog kind.
    pub(super) const fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::Tags => self.tags.count(),
            Kind::Manufacturers => self.manufacturers.len(),
            Kind::Items => self.items.len(),
            Kind::Ships => self.ships.len(),
            Kind::Commodities => self.commodities.len(),
            Kind::Blueprints => self.blueprints.len(),
            Kind::ResourceGroups => self.resource_groups.len(),
            Kind::ResourceTypes => self.resource_types.len(),
            Kind::BlueprintCategories => self.blueprint_categories.len(),
            Kind::BlueprintCategoryLists => self.blueprint_category_lists.len(),
            Kind::GameplayProperties => self.gameplay_properties.len(),
            Kind::QualityDistributions => self.quality.distributions.len(),
            Kind::QualityQuantizations => self.quality.quantizations.len(),
            Kind::BlueprintPools => self.blueprint_pools.len(),
            Kind::DismantleProcesses => self.dismantle_processes.len(),
            Kind::Paints => self.paints.len(),
            Kind::Loadouts => self.loadouts.len(),
            Kind::StarmapObjects => self.starmap_objects.len(),
            Kind::Contracts => self.contracts.len(),
            Kind::MissionLocationTemplates => self.mission_location_templates.len(),
            Kind::ContractShipSpawns => self.contract_ship_spawns.len(),
            Kind::ContractLocationQueries => self.contract_location_queries.len(),
            Kind::ContractTemplates => self.contract_templates.len(),
            Kind::MissionBrokerEntries => self.broker_definitions.len(),
            Kind::ContractNpcSpawns => self.contract_npc_spawns.len(),
            Kind::ContractEntitySpawns => self.contract_entity_spawns.len(),
            Kind::MissionPropertySets => self.mission_property_sets.len(),
            Kind::MineableElements => self.mineable_elements.len(),
            Kind::RefiningProcesses => self.refining_processes.len(),
            Kind::MineableCompositions => self.mineable_compositions.len(),
            Kind::MiningGlobalParams => self.mining_global_params.len(),
            Kind::MineableEntities => self.mineable_entities.len(),
            Kind::HarvestableProviderPresets => self.harvestable_provider_presets.len(),
            Kind::SubHarvestableMultiConfigs => self.sub_harvestable_multi_configs.len(),
            Kind::HarvestableLocations => self.harvestable_locations.len(),
            Kind::LootContainers => self.loot_containers.len(),
            Kind::Shops => self.shops.len(),
            Kind::ShopInventories => self.shop_inventories.rows.len(),
            Kind::Crafters => self.crafters.len(),
            Kind::FactionReputations
            | Kind::ReputationScopes
            | Kind::ReputationStandings
            | Kind::ReputationContexts
            | Kind::MissionTypes
            | Kind::MissionLocalities
            | Kind::ContractScenarios
            | Kind::CargoManifests => self.mission_kinds.count(kind),
            Kind::LootTables
            | Kind::LootArchetypes
            | Kind::LootPoolFilters
            | Kind::LegacyLootTables
            | Kind::LegacyLootArchetypes => self.loot_kinds.count(kind),
        }
    }
}
