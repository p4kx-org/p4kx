// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::selection::Kind;

impl Kind {
    /// Returns the rule of a kind that selects Records; a derived kind has the empty rule, which
    /// matches no path.
    #[expect(clippy::too_many_lines, reason = "a table with one entry per kind")]
    pub(crate) const fn records(self) -> KindRule {
        match self {
            Self::Manufacturers => KindRule {
                prefixes: &["libs/foundry/records/scitemmanufacturer/"],
                type_name: "SCItemManufacturer",
            },
            Self::Items => KindRule {
                prefixes: &[
                    "libs/foundry/records/entities/scitem/",
                    "libs/foundry/records/entities/legacyitems/",
                ],
                type_name: "EntityClassDefinition",
            },
            Self::Ships => KindRule {
                prefixes: &[
                    "libs/foundry/records/entities/spaceships/",
                    "libs/foundry/records/entities/groundvehicles/",
                ],
                type_name: "EntityClassDefinition",
            },
            Self::Commodities => KindRule {
                prefixes: &["libs/foundry/records/entities/commodities/"],
                type_name: "EntityClassDefinition",
            },
            Self::Blueprints => KindRule {
                prefixes: &["libs/foundry/records/crafting/blueprints/crafting/"],
                type_name: "CraftingBlueprintRecord",
            },
            Self::ResourceGroups => KindRule {
                prefixes: &["libs/foundry/records/resourcetypedatabase/"],
                type_name: "ResourceTypeGroup",
            },
            Self::BlueprintCategories => KindRule {
                prefixes: &["libs/foundry/records/crafting/blueprintcategories/"],
                type_name: "BlueprintCategoryRecord",
            },
            Self::GameplayProperties => KindRule {
                prefixes: &["libs/foundry/records/crafting/craftedproperties/"],
                type_name: "CraftingGameplayPropertyDef",
            },
            Self::QualityDistributions => KindRule {
                prefixes: &["libs/foundry/records/crafting/qualitydistribution/"],
                type_name: "CraftingQualityDistributionRecord",
            },
            Self::QualityQuantizations => KindRule {
                prefixes: &["libs/foundry/records/crafting/qualityquantization/"],
                type_name: "CraftingQualityQuantizationRecord",
            },
            Self::BlueprintPools => KindRule {
                prefixes: &["libs/foundry/records/crafting/blueprintrewards/"],
                type_name: "BlueprintPoolRecord",
            },
            Self::DismantleProcesses => KindRule {
                prefixes: &["libs/foundry/records/crafting/blueprints/dismantle/"],
                type_name: "CraftingBlueprintRecord",
            },
            Self::StarmapObjects => KindRule {
                prefixes: &["libs/foundry/records/starmap/pu/"],
                type_name: "StarMapObject",
            },
            Self::Contracts => KindRule {
                prefixes: &["libs/foundry/records/contracts/contractgenerator/"],
                type_name: "ContractGenerator",
            },
            Self::MineableElements => KindRule {
                prefixes: &["libs/foundry/records/mining/mineableelements/"],
                type_name: "MineableElement",
            },
            Self::RefiningProcesses => KindRule {
                prefixes: &["libs/foundry/records/refiningprocess/"],
                type_name: "RefiningProcess",
            },
            Self::MineableCompositions => KindRule {
                prefixes: &["libs/foundry/records/mining/rockcompositionpresets/"],
                type_name: "MineableComposition",
            },
            Self::MiningGlobalParams => KindRule {
                prefixes: &["libs/foundry/records/mining/miningglobalparams"],
                type_name: "MiningGlobalParams",
            },
            Self::HarvestableProviderPresets => KindRule {
                prefixes: &["libs/foundry/records/harvestable/providerpresets/"],
                type_name: "HarvestableProviderPreset",
            },
            Self::SubHarvestableMultiConfigs => KindRule {
                prefixes: &["libs/foundry/records/harvestable/slotpresets/"],
                type_name: "SubHarvestableMultiConfigRecord",
            },
            Self::FactionReputations => KindRule {
                prefixes: &["libs/foundry/records/factions/factionreputation/"],
                type_name: "FactionReputation",
            },
            Self::ReputationScopes => KindRule {
                prefixes: &["libs/foundry/records/reputation/scopes/"],
                type_name: "SReputationScopeParams",
            },
            Self::ReputationStandings => KindRule {
                prefixes: &["libs/foundry/records/reputation/standings/"],
                type_name: "SReputationStandingParams",
            },
            Self::ReputationContexts => KindRule {
                prefixes: &["libs/foundry/records/reputation/contexts/"],
                type_name: "SReputationContextUI",
            },
            Self::MissionTypes => KindRule {
                prefixes: &["libs/foundry/records/missiontype/"],
                type_name: "MissionType",
            },
            Self::MissionLocalities => KindRule {
                prefixes: &["libs/foundry/records/missiondata/pu_missionlocality/"],
                type_name: "MissionLocality",
            },
            Self::ContractScenarios => KindRule {
                prefixes: &["libs/foundry/records/contracts/contractscenarios/"],
                type_name: "ScenarioProgress",
            },
            Self::MissionLocationTemplates => KindRule {
                prefixes: &["libs/foundry/records/missiondata/pu_locations/"],
                type_name: "MissionLocationTemplate",
            },
            Self::ContractTemplates => KindRule {
                prefixes: &["libs/foundry/records/"],
                type_name: "ContractTemplate",
            },
            Self::MissionBrokerEntries => KindRule {
                prefixes: &["libs/foundry/records/"],
                type_name: "MissionBrokerEntry",
            },
            Self::CargoManifests => KindRule {
                prefixes: &["libs/foundry/records/cargomanifest/"],
                type_name: "CargoManifest",
            },
            Self::LootTables => KindRule {
                prefixes: &["libs/foundry/records/lootgeneration/loottables/"],
                type_name: "LootTableV3Record",
            },
            Self::LootArchetypes => KindRule {
                prefixes: &["libs/foundry/records/lootgeneration/lootarchetypes/"],
                type_name: "LootArchetypeV3Record",
            },
            Self::LootPoolFilters => KindRule {
                prefixes: &["libs/foundry/records/lootgeneration/filters/"],
                type_name: "PoolFilterRecord",
            },
            Self::LegacyLootTables => KindRule {
                prefixes: &["libs/foundry/records/lootgeneration/loottables/"],
                type_name: "LootTable",
            },
            Self::LegacyLootArchetypes => KindRule {
                prefixes: &["libs/foundry/records/lootgeneration/lootarchetypes/"],
                type_name: "LootArchetype",
            },
            Self::MineableEntities => KindRule {
                prefixes: &["libs/foundry/records/entities/mineable/"],
                type_name: "EntityClassDefinition",
            },
            Self::Crafters => KindRule {
                prefixes: &["libs/foundry/records/entities/"],
                type_name: "EntityClassDefinition",
            },
            Self::Paints
            | Self::Loadouts
            | Self::HarvestableLocations
            | Self::Shops
            | Self::ContractShipSpawns
            | Self::ContractLocationQueries
            | Self::ContractNpcSpawns
            | Self::ContractEntitySpawns
            | Self::LootContainers => KindRule {
                prefixes: &[],
                type_name: "",
            },
        }
    }
}

/// How the Records of one kind are selected: by record-path prefix and by type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct KindRule {
    /// The starts of the record path of which one must match, ignoring ASCII case and the kind
    /// of separator.
    pub prefixes: &'static [&'static str],
    /// The name of the Records' struct, matched exactly.
    pub type_name: &'static str,
}

impl KindRule {
    /// Returns whether `record_path` starts with one of the prefixes.
    #[must_use]
    pub fn matches_path(&self, record_path: &str) -> bool {
        self.prefixes
            .iter()
            .any(|prefix| starts_with_folder(record_path, prefix))
    }

    /// Returns whether a Record with `record_path` and the struct `type_name` is selected.
    pub(crate) fn selects(&self, record_path: &str, type_name: &str) -> bool {
        self.type_name == type_name && self.matches_path(record_path)
    }
}

/// Returns whether `record_path` starts with `prefix`, ignoring ASCII case and the kind of
/// separator.
pub(crate) fn starts_with_folder(record_path: &str, prefix: &str) -> bool {
    let fold = |byte: u8| {
        if byte == b'\\' {
            b'/'
        } else {
            byte.to_ascii_lowercase()
        }
    };
    record_path.len() >= prefix.len()
        && record_path
            .bytes()
            .zip(prefix.bytes())
            .all(|(have, want)| fold(have) == fold(want))
}
