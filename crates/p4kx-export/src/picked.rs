// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree};

use crate::Kind;
use crate::blueprint_category::BlueprintCategory;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::catalog_v3::ammo_definition::PreparedAmmoDefinition;
use crate::catalog_v3::blueprint_category_list::PreparedBlueprintCategoryList;
use crate::catalog_v3::explosion_definition::PreparedExplosionDefinition;
use crate::catalog_v3::faction_definition::PreparedFaction;
use crate::catalog_v3::inventory_container_definition::PreparedInventoryContainer;
use crate::catalog_v3::law_kinds::LawPick;
use crate::catalog_v3::location_vocabulary::LocationVocabularyPick;
use crate::catalog_v3::starmap_definition_kinds::StarmapDefinitionPick;
use crate::catalog_v3::starmap_object_definition::StarmapObjectRead;
use crate::contract::{
    Commodity, Crafter, HarvestableProviderPreset, Item, MineableComposition, MineableElement,
    MineableEntity, MiningGlobalParams, RefiningProcess, ResourceGroup, ResourceType, Ship,
    SubHarvestableMultiConfig,
};
use crate::crafter::crafter;
use crate::crafting_warning::CraftingWarning;
use crate::item::item;
use crate::item_class::ClassLine;
use crate::loadout::carries_ports;
use crate::loot_kinds::LootPick;
use crate::manufacturer::Manufacturer;
use crate::mining::{mineable_composition, mineable_element, refining_process};
use crate::mining_params::mining_global_params;
use crate::mission_kinds::MissionPick;
use crate::paint_port::VehicleFacts;
use crate::ship_base::ShipRead;
use crate::texts::Texts;

/// A Record a kind selected: its entity, or a development leftover of the kind.
///
/// The kinds that [`read_crafting`](crate::read_crafting) fills are not picked.
pub(crate) enum Picked<'f> {
    /// A manufacturer.
    Manufacturer(Box<Manufacturer<'f>>),
    /// An item and the configurations read from its original Record.
    Item {
        entity: Item<'f>,
        line: ClassLine,
        ports: bool,
        components: crate::catalog_v3::catalog_item_components::ItemComponentRows<'f>,
    },
    /// A ship and the vehicle facts its paints are derived from.
    Ship(Ship<'f>, VehicleFacts),
    /// A commodity.
    Commodity(Box<Commodity<'f>>),
    /// A resource group.
    ResourceGroup(ResourceGroup<'f>),
    /// A resource definition with its authored source properties.
    ResourceType(Box<ResourceType<'f>>),
    /// A blueprint category.
    BlueprintCategory(BlueprintCategory<'f>),
    /// One original category database list.
    BlueprintCategoryList(PreparedBlueprintCategoryList<'f>),
    /// One original explosion definition.
    ExplosionDefinition(Box<PreparedExplosionDefinition<'f>>),
    AmmoDefinition(Box<PreparedAmmoDefinition<'f>>),
    InventoryContainer(Box<PreparedInventoryContainer<'f>>),
    Faction(Box<PreparedFaction<'f>>),
    Law(LawPick<'f>),
    LocationVocabulary(LocationVocabularyPick<'f>),
    /// A starmap object.
    Starmap(Box<StarmapObjectRead<'f>>),
    StarmapDefinition(StarmapDefinitionPick<'f>),
    /// A mineable element.
    Mineable(MineableElement<'f>),
    /// A refining process.
    Refining(RefiningProcess<'f>),
    /// A mineable composition.
    Composition(MineableComposition<'f>),
    /// The mining parameters of a kind of miner.
    MiningParams(MiningGlobalParams<'f>),
    /// A mineable entity.
    MineableEntity(MineableEntity<'f>),
    /// A harvestable provider preset.
    Provider(HarvestableProviderPreset<'f>),
    /// A sub harvestable multi config.
    MultiConfig(SubHarvestableMultiConfig<'f>),
    /// An entity of a mission or reputation kind.
    Mission(MissionPick<'f>),
    /// An entity of a loot kind.
    Loot(LootPick<'f>),
    /// A crafting station that is no item, ship or commodity.
    Crafter(Crafter<'f>),
    /// An item, ship or commodity that is also a crafting station.
    Crafted(Box<Self>, Crafter<'f>),
    /// A development leftover of the kind, with the GUID of its Record.
    Skipped(Kind, Guid),
    /// A development leftover that is an item and a paint.
    SkippedPaint,
    /// A Record that could not be read.
    Failed(CatalogReadFailure<'f>),
    /// A warning the check of the Records found.
    Warned(CraftingWarning),
}

impl<'f> Picked<'f> {
    /// Returns the entity of `kind` the Record tree `tree` describes, its texts resolved by
    /// `texts`. `None` for a kind whose dedicated reader supplies its rows.
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive dispatch for the catalog kinds"
    )]
    pub(crate) fn entity(kind: Kind, tree: &RecordTree<'f>, texts: &Texts<'f>) -> Option<Self> {
        Some(match kind {
            Kind::Items => Self::item_entity(tree, texts),
            Kind::Crafters => return crafter(tree).map(Self::Crafter),
            Kind::MineableElements => Self::Mineable(mineable_element(tree)),
            Kind::RefiningProcesses => Self::Refining(refining_process(tree, texts)),
            Kind::MineableCompositions => Self::Composition(mineable_composition(tree, texts)),
            Kind::MiningGlobalParams => Self::MiningParams(mining_global_params(tree)),
            Kind::MissionTypes
            | Kind::MissionLocalities
            | Kind::ContractScenarios
            | Kind::CargoManifests => {
                return MissionPick::entity(kind, tree, texts).map(Self::Mission);
            }
            Kind::LootTables
            | Kind::LootArchetypes
            | Kind::LegacyLootTables
            | Kind::LegacyLootArchetypes => {
                return LootPick::entity(kind, tree).map(Self::Loot);
            }
            Kind::LootPoolFilters
            | Kind::Tags
            | Kind::Manufacturers
            | Kind::Ships
            | Kind::Commodities
            | Kind::ResourceGroups
            | Kind::ResourceTypes
            | Kind::Blueprints
            | Kind::BlueprintCategories
            | Kind::BlueprintCategoryLists
            | Kind::ReputationContexts
            | Kind::FactionReputations
            | Kind::ReputationScopes
            | Kind::ReputationStandings
            | Kind::ExplosionDefinitions
            | Kind::AmmoDefinitions
            | Kind::InventoryContainers
            | Kind::Factions
            | Kind::ReputationPerks
            | Kind::ReputationTriggerRecords
            | Kind::ReputationRewardAmounts
            | Kind::Infractions
            | Kind::InfractionSets
            | Kind::Jurisdictions
            | Kind::LocationClasses
            | Kind::LocationFunctions
            | Kind::LocationSizes
            | Kind::LocationClassRegistries
            | Kind::LocationFunctionRegistries
            | Kind::LocationSizeRegistries
            | Kind::StarmapTypes
            | Kind::StarmapPresets
            | Kind::StarmapAmenities
            | Kind::GameplayProperties
            | Kind::QualityDistributions
            | Kind::QualityQuantizations
            | Kind::BlueprintPools
            | Kind::DismantleProcesses
            | Kind::Paints
            | Kind::Loadouts
            | Kind::StarmapObjects
            | Kind::Contracts
            | Kind::MissionLocationTemplates
            | Kind::ContractShipSpawns
            | Kind::ContractLocationQueries
            | Kind::ContractNpcSpawns
            | Kind::ContractEntitySpawns
            | Kind::MissionPropertySets
            | Kind::ContractTemplates
            | Kind::MissionBrokerEntries
            | Kind::HarvestableProviderPresets
            | Kind::SubHarvestableMultiConfigs
            | Kind::HarvestableLocations
            | Kind::LootContainers
            | Kind::MineableEntities
            | Kind::Shops
            | Kind::ShopInventories => return None,
        })
    }

    fn item_entity(tree: &RecordTree<'f>, texts: &Texts<'f>) -> Self {
        let (item, line) = item(tree, texts);
        Self::Item {
            entity: item,
            line,
            ports: carries_ports(tree),
            components: crate::catalog_v3::catalog_item_components::ItemComponentRows::default(),
        }
    }

    /// Returns `self` joined with the crafting station `tree` describes when `self` is an item,
    /// ship or commodity and the entity has a `CrafterComponentParams`.
    pub(crate) fn crafted(self, tree: &RecordTree<'f>) -> Self {
        if !matches!(
            self,
            Self::Item { .. } | Self::Ship(..) | Self::Commodity(_) | Self::MineableEntity(_)
        ) {
            return self;
        }
        match crafter(tree) {
            Some(station) => Self::Crafted(Box::new(self), station),
            None => self,
        }
    }

    /// Returns the ship `build` makes of the base data `read`, or the failure when the base data
    /// could not be read: a ship is never left out without a trace.
    pub(crate) fn of_ship(
        read: Result<ShipRead, CatalogReadFailure<'f>>,
        build: impl FnOnce(&ShipRead) -> (Ship<'f>, VehicleFacts),
    ) -> Self {
        match read {
            Ok(read) => {
                let (ship, vehicle) = build(&read);
                Self::Ship(ship, vehicle)
            }
            Err(failure) => Self::Failed(failure),
        }
    }
}
