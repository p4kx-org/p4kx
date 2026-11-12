// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, Filter, Record};

pub use crate::selection_rules::KindRule;
pub(crate) use crate::selection_rules::starts_with_folder;

mod record_leftovers;

pub(crate) use record_leftovers::is_leftover;
pub use record_leftovers::{EXCLUDED_TOKENS, SHIP_EXCLUDED_TOKENS, is_kind_leftover};

/// A kind of entity in the Basetool catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
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
    /// Mission broker entries: every `MissionBrokerEntry` Record, whatever its folder, the offers
    /// of the mission board.
    MissionBrokerEntries,
    /// Contract NPC spawns: the distinct NPC spawn descriptions. Derived, so no record-path rule.
    ContractNpcSpawns,
    /// Contract entity spawns: the distinct entity spawn descriptions. Derived, so no rule either.
    ContractEntitySpawns,
    /// Authored Generator property arrays, shared by all contract rows that reference them.
    MissionPropertySets,
    /// Resource definitions: `ResourceType` Records under `resourcetypedatabase`.
    ResourceTypes,
    /// Global Tag definitions, with `TagDatabase` roots as source dependencies.
    Tags,
    /// Authored lists: every `BlueprintCategoryDatabaseRecord` under the ordinary Filter.
    BlueprintCategoryLists,
    /// Original offline shop inventory JSON files in the Archive.
    ShopInventories,
    /// Reusable physical `ExplosionRecord` definitions.
    ExplosionDefinitions,
    /// Physical ammunition configurations.
    AmmoDefinitions,
    /// All physically typed Faction Records.
    Factions,
    /// All physically typed reputation perk-list Records.
    ReputationPerks,
    /// All physical sandbox reputation trigger definition Records.
    ReputationTriggerRecords,
    /// All physical reputation reward amount Records.
    ReputationRewardAmounts,
    /// Physical authored infraction definitions.
    Infractions,
    /// Physical reusable sets of ordered infraction uses.
    InfractionSets,
    /// Physical jurisdiction definitions.
    Jurisdictions,
    /// Physical mission location class definitions.
    LocationClasses,
    /// Physical mission location function definitions.
    LocationFunctions,
    /// Physical mission location size definitions.
    LocationSizes,
    /// Physical mission location class registries.
    LocationClassRegistries,
    /// Physical mission location function registries.
    LocationFunctionRegistries,
    /// Physical mission location size registries.
    LocationSizeRegistries,
    /// All physical inventory storage Records.
    InventoryContainers,
    /// Physical shared medical tier and consumption configuration Records.
    MedicalItemTierConfigs,
    /// Physical starmap object type definitions.
    StarmapTypes,
    /// Physical starmap preset definitions.
    StarmapPresets,
    /// Physical starmap amenity definitions.
    StarmapAmenities,
}

impl Kind {
    /// Every kind, in the order of the catalog.
    pub const ALL: [Self; 72] = [
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
        Self::MissionBrokerEntries,
        Self::ContractNpcSpawns,
        Self::ContractEntitySpawns,
        Self::MissionPropertySets,
        Self::ResourceTypes,
        Self::Tags,
        Self::BlueprintCategoryLists,
        Self::ShopInventories,
        Self::ExplosionDefinitions,
        Self::AmmoDefinitions,
        Self::Factions,
        Self::ReputationPerks,
        Self::ReputationTriggerRecords,
        Self::ReputationRewardAmounts,
        Self::Infractions,
        Self::InfractionSets,
        Self::Jurisdictions,
        Self::LocationClasses,
        Self::LocationFunctions,
        Self::LocationSizes,
        Self::LocationClassRegistries,
        Self::LocationFunctionRegistries,
        Self::LocationSizeRegistries,
        Self::InventoryContainers,
        Self::MedicalItemTierConfigs,
        Self::StarmapTypes,
        Self::StarmapPresets,
        Self::StarmapAmenities,
    ];

    /// Returns the stable kind identifier, independently of its canonical dataset path.
    #[expect(
        clippy::too_many_lines,
        reason = "one stable wire name per catalog kind"
    )]
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
            Self::MissionBrokerEntries => "missionBrokerEntries",
            Self::ContractNpcSpawns => "contractNpcSpawns",
            Self::ContractEntitySpawns => "contractEntitySpawns",
            Self::MissionPropertySets => "missionPropertySets",
            Self::ResourceTypes => "resourceTypes",
            Self::Tags => "tags",
            Self::BlueprintCategoryLists => "blueprintCategoryLists",
            Self::ShopInventories => "shopInventories",
            Self::ExplosionDefinitions => "explosionDefinitions",
            Self::AmmoDefinitions => "ammoDefinitions",
            Self::Factions => "factions",
            Self::ReputationPerks => "reputationPerks",
            Self::ReputationTriggerRecords => "reputationTriggerRecords",
            Self::ReputationRewardAmounts => "reputationRewardAmounts",
            Self::Infractions => "infractions",
            Self::InfractionSets => "infractionSets",
            Self::Jurisdictions => "jurisdictions",
            Self::LocationClasses => "locationClasses",
            Self::LocationFunctions => "locationFunctions",
            Self::LocationSizes => "locationSizes",
            Self::LocationClassRegistries => "locationClassRegistries",
            Self::LocationFunctionRegistries => "locationFunctionRegistries",
            Self::LocationSizeRegistries => "locationSizeRegistries",
            Self::InventoryContainers => "inventoryContainers",
            Self::MedicalItemTierConfigs => "medicalItemTierConfigs",
            Self::StarmapTypes => "starmapTypes",
            Self::StarmapPresets => "starmapPresets",
            Self::StarmapAmenities => "starmapAmenities",
        }
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

#[cfg(test)]
mod tests;
