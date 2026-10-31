// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree};

use crate::blueprint_category::blueprint_category;
use crate::commodity::commodity;
use crate::contract::{
    BlueprintCategory, Commodity, Crafter, HarvestableProviderPreset, Item, Manufacturer,
    MineableComposition, MineableElement, MineableEntity, MiningGlobalParams, RefiningProcess,
    ResourceGroup, Ship, StarmapObject, SubHarvestableMultiConfig,
};
use crate::crafter::crafter;
use crate::crafting_warning::CraftingWarning;
use crate::item::item;
use crate::item_class::ClassLine;
use crate::loadout::carries_ports;
use crate::loot_kinds::LootPick;
use crate::manufacturer::manufacturer;
use crate::mining::{mineable_composition, mineable_element, refining_process};
use crate::mining_params::mining_global_params;
use crate::mission_kinds::MissionPick;
use crate::paint_port::VehicleFacts;
use crate::playability_rules::Evidence;
use crate::resource_group::{Groups, resource_group};
use crate::ship::ship;
use crate::ship_base::ShipRead;
use crate::texts::Texts;
use crate::{CatalogFailure, Kind};

/// A Record a kind selected: its entity, or a development leftover of the kind.
///
/// The kinds that [`read_crafting`](crate::read_crafting) fills are not picked.
pub(crate) enum Picked<'f> {
    /// A manufacturer.
    Manufacturer(Manufacturer<'f>),
    /// An item, the `Class:` line of its English description and whether it may have ports.
    Item(Item<'f>, ClassLine, bool),
    /// A ship and the vehicle facts its paints are derived from.
    Ship(Ship<'f>, VehicleFacts),
    /// A commodity.
    Commodity(Commodity<'f>),
    /// A resource group.
    ResourceGroup(ResourceGroup<'f>),
    /// A blueprint category.
    BlueprintCategory(BlueprintCategory<'f>),
    /// A starmap object.
    Starmap(StarmapObject<'f>),
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
    Failed(CatalogFailure),
    /// A warning the check of the Records found.
    Warned(CraftingWarning),
}

impl<'f> Picked<'f> {
    /// Returns the entity of `kind` the Record tree `tree` describes, its texts resolved by
    /// `texts`; a commodity may take texts from its `ResourceType` Record `resource_type` and its
    /// type from `groups`, a ship its playability from `evidence`. A ship's base data is read by
    /// `ship_base`, called for a ship only; a ship it cannot read becomes a
    /// [`Failed`](Self::Failed), never a missing Record. `None` for a kind that
    /// [`read_crafting`](crate::read_crafting) fills.
    pub(crate) fn entity(
        kind: Kind,
        tree: &RecordTree<'f>,
        texts: &Texts<'f>,
        resource_type: Option<&RecordTree<'f>>,
        groups: &Groups,
        evidence: &Evidence,
        ship_base: &dyn Fn() -> Result<ShipRead, CatalogFailure>,
    ) -> Option<Self> {
        Some(match kind {
            Kind::Manufacturers => Self::Manufacturer(manufacturer(tree, texts)),
            Kind::Items => {
                let (item, line) = item(tree, texts);
                Self::Item(item, line, carries_ports(tree))
            }
            Kind::Ships => Self::of_ship(ship_base(), |read| ship(tree, texts, evidence, read)),
            Kind::Commodities => Self::Commodity(commodity(tree, texts, resource_type, groups)),
            Kind::Crafters => return crafter(tree).map(Self::Crafter),
            Kind::ResourceGroups => Self::ResourceGroup(resource_group(tree, texts, groups)),
            Kind::BlueprintCategories => Self::BlueprintCategory(blueprint_category(tree, texts)),
            Kind::MineableElements => Self::Mineable(mineable_element(tree)),
            Kind::RefiningProcesses => Self::Refining(refining_process(tree, texts)),
            Kind::MineableCompositions => Self::Composition(mineable_composition(tree, texts)),
            Kind::MiningGlobalParams => Self::MiningParams(mining_global_params(tree)),
            Kind::FactionReputations
            | Kind::ReputationScopes
            | Kind::ReputationStandings
            | Kind::ReputationContexts
            | Kind::MissionTypes
            | Kind::MissionLocalities
            | Kind::ContractScenarios
            | Kind::CargoManifests => {
                return MissionPick::entity(kind, tree, texts).map(Self::Mission);
            }
            Kind::LootTables
            | Kind::LootArchetypes
            | Kind::LootPoolFilters
            | Kind::LegacyLootTables
            | Kind::LegacyLootArchetypes => {
                return LootPick::entity(kind, tree).map(Self::Loot);
            }
            Kind::Blueprints
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
            | Kind::ContractTemplates
            | Kind::HarvestableProviderPresets
            | Kind::SubHarvestableMultiConfigs
            | Kind::HarvestableLocations
            | Kind::LootContainers
            | Kind::MineableEntities
            | Kind::Shops => return None,
        })
    }

    /// Returns `self` joined with the crafting station `tree` describes when `self` is an item,
    /// ship or commodity and the entity has a `CrafterComponentParams`.
    pub(crate) fn crafted(self, tree: &RecordTree<'f>) -> Self {
        if !matches!(
            self,
            Self::Item(..) | Self::Ship(..) | Self::Commodity(_) | Self::MineableEntity(_)
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
        read: Result<ShipRead, CatalogFailure>,
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
