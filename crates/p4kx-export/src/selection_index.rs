// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::selection::Kind;

impl Kind {
    /// Returns the position of the kind in [`Kind::ALL`].
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Manufacturers => 0,
            Self::Items => 1,
            Self::Ships => 2,
            Self::Commodities => 3,
            Self::Blueprints => 4,
            Self::ResourceGroups => 5,
            Self::BlueprintCategories => 6,
            Self::GameplayProperties => 7,
            Self::QualityDistributions => 8,
            Self::QualityQuantizations => 9,
            Self::BlueprintPools => 10,
            Self::DismantleProcesses => 11,
            Self::Paints => 12,
            Self::Loadouts => 13,
            Self::StarmapObjects => 14,
            Self::Contracts => 15,
            Self::MineableElements => 16,
            Self::RefiningProcesses => 17,
            Self::MineableCompositions => 18,
            Self::MiningGlobalParams => 19,
            Self::HarvestableProviderPresets => 20,
            Self::SubHarvestableMultiConfigs => 21,
            Self::HarvestableLocations => 22,
            Self::Shops => 23,
            Self::FactionReputations => 24,
            Self::ReputationScopes => 25,
            Self::ReputationStandings => 26,
            Self::ReputationContexts => 27,
            Self::MissionTypes => 28,
            Self::MissionLocalities => 29,
            Self::ContractScenarios => 30,
            Self::LootTables => 31,
            Self::LootArchetypes => 32,
            Self::LootPoolFilters => 33,
            Self::LootContainers => 34,
            Self::MineableEntities => 35,
            Self::LegacyLootTables => 36,
            Self::LegacyLootArchetypes => 37,
            Self::Crafters => 38,
            Self::CargoManifests => 39,
            Self::MissionLocationTemplates => 40,
            Self::ContractShipSpawns => 41,
            Self::ContractLocationQueries => 42,
            Self::ContractTemplates => 43,
            Self::MissionBrokerEntries => 44,
            Self::ContractNpcSpawns => 45,
            Self::ContractEntitySpawns => 46,
        }
    }
}
