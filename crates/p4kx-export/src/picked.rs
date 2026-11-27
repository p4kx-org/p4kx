// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::blueprint_category::BlueprintCategory;
use crate::catalog_projections::ship_base_facts::ShipBaseFacts;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::catalog_v3::ammo_definition::PreparedAmmoDefinition;
use crate::catalog_v3::blueprint_category_list::PreparedBlueprintCategoryList;
use crate::catalog_v3::cargo_globals::{PreparedCargoLoadingGlobal, PreparedShopCommodityGlobal};
use crate::catalog_v3::catalog_item_components::ItemComponentRows;
use crate::catalog_v3::catalog_item_components::item_definition::ItemDefinition;
use crate::catalog_v3::catalog_item_components::resolution::MagazineDependency;
use crate::catalog_v3::catalog_resource_network_read::resource_network_global::PreparedResourceNetworkGlobal;
use crate::catalog_v3::explosion_definition::PreparedExplosionDefinition;
use crate::catalog_v3::faction_definition::PreparedFaction;
use crate::catalog_v3::industry_modules::crafter_definition::PreparedCrafter;
use crate::catalog_v3::inventory_container_definition::PreparedInventoryContainer;
use crate::catalog_v3::law_kinds::LawPick;
use crate::catalog_v3::location_vocabulary::LocationVocabularyPick;
use crate::catalog_v3::loot_definition_catalog::LootDefinition;
use crate::catalog_v3::medical::medical_tier_config::PreparedMedicalTier;
use crate::catalog_v3::npc_spawn_definitions::NpcSpawnDefinitionsPick;
use crate::catalog_v3::procedural_planet_audio::PreparedPlanetAudio;
use crate::catalog_v3::starmap_definition_kinds::StarmapDefinitionPick;
use crate::catalog_v3::starmap_object_definition::StarmapObjectRead;
use crate::contract::{
    Commodity, HarvestableProviderPreset, Item, MineableComposition, MineableElement,
    MineableEntity, MiningGlobalParams, RefiningProcess, ResourceGroup, ResourceType, Ship,
    SubHarvestableMultiConfig,
};
use crate::crafting_warning::CraftingWarning;
use crate::item_class::ClassLine;
use crate::loot_kinds::LootPick;
use crate::manufacturer::Manufacturer;
use crate::mission_kinds::MissionPick;
use crate::paint_port::VehicleFacts;
use crate::ship_base::ShipRead;
use crate::{CatalogError, Kind};

pub(crate) struct PickedItem<'f> {
    pub(crate) entity: Item<'f>,
    pub(crate) line: ClassLine,
    pub(crate) ports: Option<crate::loadout::Wanted>,
    pub(crate) components: ItemComponentRows<'f>,
    pub(crate) definition: ItemDefinition<'f>,
}

/// A Record a kind selected: its entity, or a development leftover of the kind.
///
/// The kinds that [`read_crafting`](crate::read_crafting) fills are not picked.
pub(crate) enum Picked<'f> {
    /// A manufacturer.
    Manufacturer(Box<Manufacturer<'f>>),
    /// An item and the configurations read from its original Record.
    Item(Box<PickedItem<'f>>),
    /// A ship and the vehicle facts its paints are derived from.
    Ship(Box<Ship<'f>>, VehicleFacts, Box<ShipBaseFacts>),
    /// A commodity.
    Commodity(Box<Commodity<'f>>),
    /// A resource group.
    ResourceGroup(Box<ResourceGroup<'f>>),
    /// A resource definition with its authored source properties.
    ResourceType(Box<ResourceType<'f>>),
    /// A blueprint category.
    BlueprintCategory(BlueprintCategory<'f>),
    /// One original category database list.
    BlueprintCategoryList(PreparedBlueprintCategoryList<'f>),
    /// One original explosion definition.
    ExplosionDefinition(Box<PreparedExplosionDefinition<'f>>),
    AmmoDefinition(Box<PreparedAmmoDefinition<'f>>),
    AmmoMagazineDependency(Box<MagazineDependency<'f>>),
    InventoryContainer(Box<PreparedInventoryContainer<'f>>),
    MedicalItemTierConfig(Box<PreparedMedicalTier<'f>>),
    ProceduralPlanetAudio(Box<PreparedPlanetAudio<'f>>),
    ResourceNetworkGlobal(Box<PreparedResourceNetworkGlobal<'f>>),
    CargoLoadingGlobal(Box<PreparedCargoLoadingGlobal<'f>>),
    ShopCommodityGlobal(Box<PreparedShopCommodityGlobal<'f>>),
    FactionIdentity(
        Box<crate::catalog_v3::faction_identity_definition::PreparedFactionIdentity<'f>>,
    ),
    Faction(Box<PreparedFaction<'f>>),
    Law(LawPick<'f>),
    LocationVocabulary(LocationVocabularyPick<'f>),
    NpcSpawnDefinitions(NpcSpawnDefinitionsPick<'f>),
    /// A starmap object.
    Starmap(Box<StarmapObjectRead<'f>>),
    StarmapDefinition(StarmapDefinitionPick<'f>),
    /// A mineable element.
    Mineable(Box<MineableElement>),
    /// A refining process.
    Refining(Box<RefiningProcess<'f>>),
    /// A mineable composition.
    Composition(Box<MineableComposition>),
    /// The mining parameters of a kind of miner.
    MiningParams(Box<MiningGlobalParams<'f>>),
    /// A mineable entity.
    MineableEntity(MineableEntity<'f>),
    /// A harvestable provider preset.
    Provider(Box<HarvestableProviderPreset>),
    /// A physical multi config with an optional slot-preset projection.
    MultiConfig {
        definition: Box<LootDefinition<'f>>,
        projection: Option<SubHarvestableMultiConfig<'f>>,
    },
    /// An entity of a mission or reputation kind.
    Mission(MissionPick<'f>),
    /// An entity of a loot kind.
    Loot(LootPick<'f>),
    /// A crafting station that is no item, ship or commodity.
    Crafter(Box<PreparedCrafter<'f>>),
    /// An item, ship or commodity that is also a crafting station.
    Crafted(Box<Self>, Box<PreparedCrafter<'f>>),
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
    /// Returns the ship `build` makes of the base data `read`, or the failure when the base data
    /// could not be read: a ship is never left out without a trace.
    pub(crate) fn of_ship(
        read: Result<ShipRead, CatalogReadFailure<'f>>,
        build: impl FnOnce(&ShipRead) -> Result<(Ship<'f>, VehicleFacts, ShipBaseFacts), CatalogError>,
    ) -> Result<Self, CatalogError> {
        Ok(match read {
            Ok(read) => {
                let (ship, vehicle, facts) = build(&read)?;
                Self::Ship(Box::new(ship), vehicle, Box::new(facts))
            }
            Err(failure) => Self::Failed(failure),
        })
    }
}
