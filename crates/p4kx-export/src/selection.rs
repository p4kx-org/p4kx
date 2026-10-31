// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use p4kx_extraction::{DataForgeError, Filter, Record};

pub use crate::selection_rules::KindRule;
pub(crate) use crate::selection_rules::starts_with_folder;

/// The tokens that mark a development leftover: a Record whose record path or name holds one of
/// them as a whole word, ignoring ASCII case, is left out and counted as skipped.
///
/// Words are the runs of ASCII letters and digits, so `paint_test_01` holds `test` while
/// `contestant` does not.
pub const EXCLUDED_TOKENS: [&str; 6] = ["test", "template", "dummy", "debug", "deprecated", "dev"];

/// The leftover tokens of a contract template: [`EXCLUDED_TOKENS`] without `template`, which
/// live templates such as `handyman_template` carry.
const CONTRACT_TEMPLATE_EXCLUDED_TOKENS: [&str; 5] =
    ["test", "dummy", "debug", "deprecated", "dev"];

/// The extra tokens of a leftover ship.
///
/// A Record of the ships kind whose record path or name holds one of them as a whole word, ignoring
/// ASCII case, is left out and counted as skipped, as for [`EXCLUDED_TOKENS`]. The other kinds keep
/// their `display` and `temp` Records.
pub const SHIP_EXCLUDED_TOKENS: [&str; 3] = ["temp", "display", "hijacked"];

/// A kind of entity in the Basetool catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Kind {
    /// Manufacturers: `SCItemManufacturer` Records.
    Manufacturers,
    /// Items: `EntityClassDefinition` Records under `entities/scitem` and `entities/legacyitems`.
    Items,
    /// Ships and ground vehicles: `EntityClassDefinition` Records under `entities/spaceships` and
    /// `entities/groundvehicles`.
    Ships,
    /// Commodities: `EntityClassDefinition` Records under `entities/commodities`.
    Commodities,
    /// Blueprints: `CraftingBlueprintRecord` Records under `crafting/blueprints/crafting`.
    Blueprints,
    /// Resource groups: `ResourceTypeGroup` Records under `resourcetypedatabase`.
    ResourceGroups,
    /// Blueprint categories: `BlueprintCategoryRecord` Records under
    /// `crafting/blueprintcategories`.
    BlueprintCategories,
    /// Gameplay property definitions: `CraftingGameplayPropertyDef` Records under
    /// `crafting/craftedproperties`.
    GameplayProperties,
    /// Quality distributions: `CraftingQualityDistributionRecord` and
    /// `CraftingQualityLocationOverrideRecord` Records under `crafting/qualitydistribution`.
    QualityDistributions,
    /// Quality quantizations: `CraftingQualityQuantizationRecord` Records under
    /// `crafting/qualityquantization`, each linked to the `ResourceType`s that use it.
    QualityQuantizations,
    /// Blueprint pools: `BlueprintPoolRecord` Records under `crafting/blueprintrewards`.
    BlueprintPools,
    /// Dismantle processes: `CraftingBlueprintRecord` Records under
    /// `crafting/blueprints/dismantle` whose process is a `GenericCraftingProcess_Dismantle`.
    DismantleProcesses,
    /// Paints: the items whose `AttachDef.Type` is `Paints`, with the ships they fit. Derived from
    /// the items and ships, so no record-path rule of its own.
    Paints,
    /// Loadouts: the ports of every ship and of every item that has ports, with the Entity the
    /// Default Loadout attaches to each. Derived from the items and ships, so no record-path rule
    /// of its own.
    Loadouts,
    /// Starmap objects: `StarMapObject` Records under `starmap/pu`, each with the position the
    /// system containers place it at.
    StarmapObjects,
    /// Contracts: every contract and every sub contract of the `ContractGenerator` Records under
    /// `contracts/contractgenerator`, one entry each.
    Contracts,
    /// Mineable elements: `MineableElement` Records under `mining/mineableelements`.
    MineableElements,
    /// Refining processes: `RefiningProcess` Records under `refiningprocess`.
    RefiningProcesses,
    /// Mineable compositions: `MineableComposition` Records under
    /// `mining/rockcompositionpresets`.
    MineableCompositions,
    /// Mining global parameters: `MiningGlobalParams` Records under `mining`.
    MiningGlobalParams,
    /// Harvestable provider presets: `HarvestableProviderPreset` Records under
    /// `harvestable/providerpresets`.
    HarvestableProviderPresets,
    /// Sub harvestable multi configs: `SubHarvestableMultiConfigRecord` Records under
    /// `harvestable/slotpresets`.
    SubHarvestableMultiConfigs,
    /// Harvestable locations: the containers whose `.soc` names a provider preset, with the
    /// starmap object they hang below. Derived from the system containers, so no record-path rule
    /// of its own.
    HarvestableLocations,
    /// Shops: the `ShopBackup` entries of the shop registry `LayerBackups.xml`, with kind and brand
    /// from the shop containers. Derived from Archive files, so no record-path rule of its own.
    Shops,
    /// Factions: `FactionReputation` Records under `factions/factionreputation`.
    FactionReputations,
    /// Reputation scopes: `SReputationScopeParams` Records under `reputation/scopes`.
    ReputationScopes,
    /// Reputation standings: `SReputationStandingParams` Records under `reputation/standings`.
    ReputationStandings,
    /// Reputation contexts: `SReputationContextUI` Records under `reputation/contexts`.
    ReputationContexts,
    /// Mission types: `MissionType` Records under `missiontype`.
    MissionTypes,
    /// Mission localities: `MissionLocality` Records under `missiondata/pu_missionlocality`.
    MissionLocalities,
    /// Contract scenarios: `ScenarioProgress` Records under `contracts/contractscenarios`.
    ContractScenarios,
    /// Loot tables: `LootTableV3Record` Records under `lootgeneration/loottables`.
    LootTables,
    /// Loot archetypes: `LootArchetypeV3Record` Records under `lootgeneration/lootarchetypes`.
    LootArchetypes,
    /// Loot pool filters: `PoolFilterRecord` Records under `lootgeneration/filters`.
    LootPoolFilters,
    /// Loot containers: the containers whose placing entity carries a multi config, read from the
    /// entity files of the containers below the system containers. Derived from Archive files, so
    /// no record-path rule of its own.
    LootContainers,
    /// Mineable entities: `EntityClassDefinition` Records under `entities/mineable`, the rocks and
    /// deposits with their scan signature.
    MineableEntities,
    /// Older loot tables: `LootTable` Records under `lootgeneration/loottables`, which the
    /// `lootTable` of a slot's `lootConfig` links to.
    LegacyLootTables,
    /// Older loot archetypes: `LootArchetype` Records under `lootgeneration/lootarchetypes`.
    LegacyLootArchetypes,
    /// Crafters: the entities whose `Components` hold a `CrafterComponentParams`. The rule takes
    /// the `EntityClassDefinition` Records under `entities` that no earlier kind takes, so it
    /// comes last; an item, ship or commodity that is a crafter is read from its own Record.
    /// [`Kind::rule`] alone does not select a crafter: the Record must also hold the component.
    Crafters,
    /// Cargo manifests: `CargoManifest` Records under `cargomanifest`.
    CargoManifests,
    /// Mission location templates: `MissionLocationTemplate` Records under
    /// `missiondata/pu_locations`, what a location query of a contract matches by tags.
    MissionLocationTemplates,
    /// Contract ship spawns: the distinct ship spawn descriptions the contracts state. Derived from
    /// the contracts, so no record-path rule of its own.
    ContractShipSpawns,
    /// Contract location queries: the distinct location queries the contracts state. Derived from
    /// the contracts, so no record-path rule of its own.
    ContractLocationQueries,
    /// Contract templates: every `ContractTemplate` Record, whatever its folder, the structure a
    /// contract names in `templateGuid`.
    ContractTemplates,
}

impl Kind {
    /// Every kind, in the order of the catalog.
    pub const ALL: [Self; 44] = [
        Self::Manufacturers,
        Self::Items,
        Self::Ships,
        Self::Commodities,
        Self::Blueprints,
        Self::ResourceGroups,
        Self::BlueprintCategories,
        Self::GameplayProperties,
        Self::QualityDistributions,
        Self::QualityQuantizations,
        Self::BlueprintPools,
        Self::DismantleProcesses,
        Self::Paints,
        Self::Loadouts,
        Self::StarmapObjects,
        Self::Contracts,
        Self::MineableElements,
        Self::RefiningProcesses,
        Self::MineableCompositions,
        Self::MiningGlobalParams,
        Self::HarvestableProviderPresets,
        Self::SubHarvestableMultiConfigs,
        Self::HarvestableLocations,
        Self::Shops,
        Self::FactionReputations,
        Self::ReputationScopes,
        Self::ReputationStandings,
        Self::ReputationContexts,
        Self::MissionTypes,
        Self::MissionLocalities,
        Self::ContractScenarios,
        Self::LootTables,
        Self::LootArchetypes,
        Self::LootPoolFilters,
        Self::LootContainers,
        Self::MineableEntities,
        Self::LegacyLootTables,
        Self::LegacyLootArchetypes,
        Self::Crafters,
        Self::CargoManifests,
        Self::MissionLocationTemplates,
        Self::ContractShipSpawns,
        Self::ContractLocationQueries,
        Self::ContractTemplates,
    ];

    /// Returns the name of the kind's array in the catalog, e.g. `manufacturers`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Manufacturers => "manufacturers",
            Self::Items => "items",
            Self::Ships => "ships",
            Self::Commodities => "commodities",
            Self::Blueprints => "blueprints",
            Self::ResourceGroups => "resourceGroups",
            Self::BlueprintCategories => "blueprintCategories",
            Self::GameplayProperties => "gameplayProperties",
            Self::QualityDistributions => "qualityDistributions",
            Self::QualityQuantizations => "qualityQuantizations",
            Self::BlueprintPools => "blueprintPools",
            Self::DismantleProcesses => "dismantleProcesses",
            Self::Paints => "paints",
            Self::Loadouts => "loadouts",
            Self::StarmapObjects => "starmapObjects",
            Self::Contracts => "contracts",
            Self::MineableElements => "mineableElements",
            Self::RefiningProcesses => "refiningProcesses",
            Self::MineableCompositions => "mineableCompositions",
            Self::MiningGlobalParams => "miningGlobalParams",
            Self::HarvestableProviderPresets => "harvestableProviderPresets",
            Self::SubHarvestableMultiConfigs => "subHarvestableMultiConfigs",
            Self::HarvestableLocations => "harvestableLocations",
            Self::Shops => "shops",
            Self::FactionReputations => "factionReputations",
            Self::ReputationScopes => "reputationScopes",
            Self::ReputationStandings => "reputationStandings",
            Self::ReputationContexts => "reputationContexts",
            Self::MissionTypes => "missionTypes",
            Self::MissionLocalities => "missionLocalities",
            Self::ContractScenarios => "contractScenarios",
            Self::LootTables => "lootTables",
            Self::LootArchetypes => "lootArchetypes",
            Self::LootPoolFilters => "lootPoolFilters",
            Self::LootContainers => "lootContainers",
            Self::MineableEntities => "mineableEntities",
            Self::LegacyLootTables => "legacyLootTables",
            Self::LegacyLootArchetypes => "legacyLootArchetypes",
            Self::Crafters => "crafters",
            Self::CargoManifests => "cargoManifests",
            Self::MissionLocationTemplates => "missionLocationTemplates",
            Self::ContractShipSpawns => "contractShipSpawns",
            Self::ContractLocationQueries => "contractLocationQueries",
            Self::ContractTemplates => "contractTemplates",
        }
    }

    /// Returns whether the kind is read by [`read_crafting`](crate::read_crafting) or by the
    /// contract reader rather than by the generic selection of the catalog build.
    pub(crate) const fn is_crafting(self) -> bool {
        matches!(
            self,
            Self::Blueprints
                | Self::GameplayProperties
                | Self::QualityDistributions
                | Self::QualityQuantizations
                | Self::BlueprintPools
                | Self::DismantleProcesses
                | Self::Contracts
                | Self::MissionLocationTemplates
                | Self::ContractTemplates
        )
    }

    /// Returns the rule that selects the kind's Records, `None` for a [derived](Self::is_derived)
    /// kind: [`Kind::Paints`] and [`Kind::Loadouts`] are made of the entities of other kinds and
    /// [`Kind::HarvestableLocations`] and [`Kind::Shops`] read Archive files, not Records.
    #[must_use]
    pub const fn rule(self) -> Option<KindRule> {
        if self.is_derived() {
            None
        } else {
            Some(self.records())
        }
    }

    /// Returns the first kind in [`Kind::ALL`] one of whose prefixes `record_path` starts with,
    /// and never a derived kind. The prefixes of [`Kind::Crafters`] overlap those of the entity
    /// kinds; only the order of [`Kind::ALL`] resolves it. Kinds that share a folder are told
    /// apart by [`Self::by_record`].
    pub(crate) fn by_path(record_path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| {
            kind.rule()
                .is_some_and(|rule| rule.matches_path(record_path))
        })
    }

    /// Returns the kind that selects a Record with `record_path` and the struct `type_name`; the
    /// kinds that share a folder (the V3 and the older loot Records) differ by the struct.
    pub(crate) fn by_record(record_path: &str, type_name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| {
            kind.rule()
                .is_some_and(|rule| rule.selects(record_path, type_name))
        })
    }

    /// Returns whether the kind is derived from the entities of other kinds and selects no Records
    /// of its own.
    #[must_use]
    pub const fn is_derived(self) -> bool {
        matches!(
            self,
            Self::Paints
                | Self::Loadouts
                | Self::HarvestableLocations
                | Self::Shops
                | Self::LootContainers
                | Self::ContractShipSpawns
                | Self::ContractLocationQueries
        )
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What the blueprint selection makes of one Record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlueprintSelection {
    /// Not a blueprint of the catalog, or one the Filter does not match.
    Outside,
    /// A blueprint of the catalog.
    Kept,
    /// A blueprint left out as a development leftover.
    Leftover,
}

/// Returns what the blueprint selection makes of `record`: the one rule behind the `blueprints`
/// kind of the catalog and the blueprints of [`read_crafting`](crate::read_crafting).
///
/// # Errors
///
/// Returns the error of the Record's path, type or name when it cannot be read.
pub(crate) fn selects_blueprint(
    record: &Record<'_>,
    filter: Option<&Filter>,
) -> Result<BlueprintSelection, DataForgeError> {
    let rule = Kind::Blueprints.records();
    let path = record.record_path()?;
    if !rule.matches_path(path)
        || record.type_name()? != rule.type_name
        || filter.is_some_and(|filter| !filter.matches(path))
    {
        return Ok(BlueprintSelection::Outside);
    }
    Ok(if is_leftover(path, record.name()?) {
        BlueprintSelection::Leftover
    } else {
        BlueprintSelection::Kept
    })
}

fn has_word(record_path: &str, name: &str, tokens: &[&str]) -> bool {
    [record_path, name].into_iter().any(|text| {
        text.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| tokens.iter().any(|token| word.eq_ignore_ascii_case(token)))
    })
}

/// Returns whether a Record with `record_path` and `name` is a development leftover.
pub(crate) fn is_leftover(record_path: &str, name: &str) -> bool {
    has_word(record_path, name, &EXCLUDED_TOKENS)
}

/// Returns whether a Record of `kind` with `record_path` and `name` is a development leftover:
/// one of [`is_leftover`], or a ship with a [`SHIP_EXCLUDED_TOKENS`] word. A contract template
/// is a leftover by the words of [`EXCLUDED_TOKENS`] except `template`, which names live ones.
pub(crate) fn is_kind_leftover(kind: Kind, record_path: &str, name: &str) -> bool {
    match kind {
        Kind::ContractTemplates => has_word(record_path, name, &CONTRACT_TEMPLATE_EXCLUDED_TOKENS),
        Kind::Ships => {
            is_leftover(record_path, name) || has_word(record_path, name, &SHIP_EXCLUDED_TOKENS)
        }
        _ => is_leftover(record_path, name),
    }
}

#[cfg(test)]
mod tests;
