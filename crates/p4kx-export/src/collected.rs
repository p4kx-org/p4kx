// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::Guid;

use crate::blueprint_catalog_rows::BlueprintRow;
use crate::blueprint_category::BlueprintCategory;
use crate::broker_definition::BrokerDefinition;
use crate::broker_entry::BrokerProjection;
use crate::catalog_crafting::CraftingKinds;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::catalog_v3::blueprint_category_list::PreparedBlueprintCategoryList;
use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyDefinition;
use crate::contract::{
    BlueprintPool, Commodity, ContractEntry, Crafter, CraftingGlobal, DismantleProcess,
    HarvestableLocation, HarvestableProviderPreset, Item, Loadout, LootContainer,
    MineableComposition, MineableElement, MineableEntity, MiningGlobalParams, Paint,
    RefiningProcess, ResourceGroup, ResourceType, Ship, Shop, StarmapObject,
    SubHarvestableMultiConfig,
};
use crate::contract_missions::ContractWithTitles;
use crate::crafting_warning::CraftingWarning;
use crate::entity_spawn_model::EntitySpawnPool;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::location_template::{LocationTemplates, MissionLocationTemplate};
use crate::loot_kinds::LootKinds;
use crate::manufacturer::Manufacturer;
use crate::mission_generator::GeneratorDefinition;
use crate::mission_kinds::MissionKinds;
use crate::mission_property_set::MissionPropertySet;
use crate::mission_titles::MissionTitles;
use crate::npc_spawn_model::NpcSpawnPool;
use crate::paint_port::VehicleFacts;
use crate::picked::Picked;
use crate::playability::PlayabilitySummary;
use crate::quality_catalog::QualityRows;
use crate::record_read::KindRead;
use crate::shop_inventories::ShopInventoryTable;
use crate::shop_inventory_resolution::ShopInventoryResolution;
use crate::spawn_model::{LocationQuery, ShipSpawnPool};
use crate::spawn_pools::SpawnPools;
use crate::template_definition::TemplateDefinition;
use crate::template_issues::Unknowns;
use crate::template_rows::TemplateConfigurations;
use crate::texts::Texts;
use crate::{ItemClassSummary, Kind, KindSummary};

#[path = "collected_counts.rs"]
mod counts;
#[path = "collected_order.rs"]
mod order;

/// The entities of every kind, ordered by GUID, with the counts of the summary.
pub(crate) struct Collected<'f> {
    pub(crate) ledger: ExportLedger,
    pub(crate) tags: crate::catalog_v3::tag_catalog::TagRows<'f>,
    pub(crate) manufacturers: Vec<Manufacturer<'f>>,
    pub(crate) items: Vec<Item<'f>>,
    pub(crate) ships: Vec<Ship<'f>>,
    pub(crate) commodities: Vec<Commodity<'f>>,
    pub(crate) blueprints: Vec<BlueprintRow<'f>>,
    pub(crate) missing_blueprint_categories: HashSet<Guid>,
    pub(crate) resource_groups: Vec<ResourceGroup<'f>>,
    pub(crate) resource_group_members:
        Vec<crate::catalog_v3::resource_group_catalog::ResourceGroupMembers>,
    pub(crate) resource_types: Vec<ResourceType<'f>>,
    pub(crate) blueprint_categories: Vec<BlueprintCategory<'f>>,
    pub(crate) blueprint_category_lists: Vec<PreparedBlueprintCategoryList<'f>>,
    pub(crate) gameplay_properties: Vec<GameplayPropertyDefinition<'f>>,
    pub(crate) quality: QualityRows<'f>,
    pub(crate) blueprint_pools: Vec<BlueprintPool<'f>>,
    pub(crate) dismantle_processes: Vec<DismantleProcess<'f>>,
    pub(crate) crafting_global: CraftingGlobal,
    pub(crate) paints: Vec<Paint>,
    pub(crate) loadouts: Vec<Loadout>,
    pub(crate) contracts: Vec<ContractEntry<'f>>,
    pub(crate) mission_titles: Vec<MissionTitles<'f>>,
    pub(crate) contract_generators: Vec<GeneratorDefinition<'f>>,
    pub(crate) mission_location_templates: Vec<MissionLocationTemplate<'f>>,
    pub(crate) contract_ship_spawns: Vec<ShipSpawnPool>,
    pub(crate) contract_location_queries: Vec<LocationQuery>,
    pub(crate) contract_templates: Vec<TemplateDefinition<'f>>,
    pub(crate) template_configurations: TemplateConfigurations<'f>,
    pub(crate) broker_projections: Vec<BrokerProjection<'f>>,
    pub(crate) broker_definitions: Vec<BrokerDefinition<'f>>,
    pub(crate) contract_npc_spawns: Vec<NpcSpawnPool<'f>>,
    pub(crate) contract_entity_spawns: Vec<EntitySpawnPool>,
    pub(crate) mission_property_sets: Vec<MissionPropertySet<'f>>,
    pub(crate) port_items: Vec<crate::loadout::Wanted>,
    pub(crate) starmap_objects: Vec<StarmapObject<'f>>,
    pub(crate) mineable_elements: Vec<MineableElement<'f>>,
    pub(crate) refining_processes: Vec<RefiningProcess<'f>>,
    pub(crate) mineable_compositions: Vec<MineableComposition<'f>>,
    pub(crate) mining_global_params: Vec<MiningGlobalParams<'f>>,
    pub(crate) mineable_entities: Vec<MineableEntity<'f>>,
    pub(crate) harvestable_provider_presets: Vec<HarvestableProviderPreset<'f>>,
    pub(crate) sub_harvestable_multi_configs: Vec<SubHarvestableMultiConfig<'f>>,
    pub(crate) harvestable_locations: Vec<HarvestableLocation>,
    pub(crate) loot_containers: Vec<LootContainer>,
    pub(crate) shops: Vec<Shop>,
    pub(crate) shop_inventories: ShopInventoryTable,
    pub(crate) shop_inventory_resolution: CatalogTable<ShopInventoryResolution>,
    pub(crate) crafters: Vec<Crafter<'f>>,
    pub(crate) mission_kinds: MissionKinds<'f>,
    pub(crate) loot_kinds: LootKinds<'f>,
    pub(crate) vehicles: Vec<VehicleFacts>,
    pub(crate) item_classes: ItemClassSummary,
    pub(crate) playability: PlayabilitySummary,
    pub(crate) failures: Vec<CatalogReadFailure<'f>>,
    pub(crate) warnings: Vec<CraftingWarning>,
    skipped: [usize; Kind::ALL.len()],
}

impl<'f> Collected<'f> {
    /// Collects picked Records and crafting rows in GUID order.
    pub(crate) fn new(picked: Vec<Picked<'f>>, crafting: CraftingKinds<'f>) -> Self {
        let mut collected = Self::from_crafting(crafting);
        let mut missing_categories = HashSet::new();
        for pick in picked {
            collected.take(pick, &mut missing_categories);
        }
        missing_categories.extend(
            collected
                .ledger
                .sources
                .iter()
                .filter(|source| {
                    source.kind == Some(Kind::BlueprintCategories)
                        && matches!(
                            source.role,
                            ExportSourceRole::Selected | ExportSourceRole::SelectionCandidate
                        )
                        && source.disposition.failed()
                })
                .map(|source| source.guid),
        );
        collected.missing_blueprint_categories = missing_categories;
        collected.order();
        collected
    }

    fn from_crafting(crafting: CraftingKinds<'f>) -> Self {
        let skipped = Self::crafting_skipped(&crafting);
        Self {
            ledger: crafting.ledger,
            tags: crate::catalog_v3::tag_catalog::TagRows::default(),
            manufacturers: Vec::new(),
            items: Vec::new(),
            ships: Vec::new(),
            commodities: Vec::new(),
            blueprints: crafting.blueprints,
            missing_blueprint_categories: HashSet::new(),
            resource_groups: Vec::new(),
            resource_group_members: Vec::new(),
            resource_types: Vec::new(),
            blueprint_categories: Vec::new(),
            blueprint_category_lists: Vec::new(),
            gameplay_properties: crafting.properties,
            quality: crafting.quality,
            blueprint_pools: crafting.pools,
            dismantle_processes: crafting.dismantle,
            crafting_global: crafting.global,
            paints: Vec::new(),
            loadouts: Vec::new(),
            contracts: Vec::new(),
            mission_titles: Vec::new(),
            contract_generators: Vec::new(),
            mission_location_templates: Vec::new(),
            contract_ship_spawns: Vec::new(),
            contract_location_queries: Vec::new(),
            contract_templates: Vec::new(),
            template_configurations: TemplateConfigurations::default(),
            broker_projections: Vec::new(),
            broker_definitions: Vec::new(),
            contract_npc_spawns: Vec::new(),
            contract_entity_spawns: Vec::new(),
            mission_property_sets: Vec::new(),
            port_items: Vec::new(),
            starmap_objects: Vec::new(),
            mineable_elements: Vec::new(),
            refining_processes: Vec::new(),
            mineable_compositions: Vec::new(),
            mining_global_params: Vec::new(),
            mineable_entities: Vec::new(),
            harvestable_provider_presets: Vec::new(),
            sub_harvestable_multi_configs: Vec::new(),
            harvestable_locations: Vec::new(),
            loot_containers: Vec::new(),
            shops: Vec::new(),
            shop_inventories: ShopInventoryTable::default(),
            shop_inventory_resolution: CatalogTable::empty(DatasetAvailability::Absent),
            crafters: Vec::new(),
            mission_kinds: MissionKinds::default(),
            loot_kinds: LootKinds::default(),
            vehicles: Vec::new(),
            item_classes: ItemClassSummary::default(),
            playability: PlayabilitySummary::default(),
            failures: crafting.failures,
            warnings: crafting.warnings,
            skipped,
        }
    }

    /// Files `pick` under its kind; the GUIDs of the skipped blueprint categories go to
    /// `missing_categories`.
    fn take(&mut self, pick: Picked<'f>, missing_categories: &mut HashSet<Guid>) {
        match pick {
            Picked::Manufacturer(entity) => self.manufacturers.push(*entity),
            Picked::Item(entity, line, ports) => {
                self.item_classes.count(line);
                if ports {
                    self.port_items.push(crate::loadout::Wanted {
                        guid: entity.guid,
                        kind: crate::contract_loadout::EntityKind::Item,
                        record_index: entity.source_record_index,
                    });
                }
                self.playability.items.count(entity.playability);
                self.items.push(entity);
            }
            Picked::Ship(entity, vehicle) => {
                self.playability.ships.count(entity.playability);
                self.ships.push(entity);
                self.vehicles.push(vehicle);
            }
            Picked::Commodity(entity) => self.commodities.push(*entity),
            Picked::ResourceGroup(entity) => self.resource_groups.push(entity),
            Picked::ResourceType(entity) => self.resource_types.push(*entity),
            Picked::BlueprintCategory(entity) => self.blueprint_categories.push(entity),
            Picked::BlueprintCategoryList(entity) => self.blueprint_category_lists.push(entity),
            Picked::Starmap(entity) => self.starmap_objects.push(entity),
            Picked::Mineable(entity) => self.mineable_elements.push(entity),
            Picked::Refining(entity) => self.refining_processes.push(entity),
            Picked::Composition(entity) => self.mineable_compositions.push(entity),
            Picked::MiningParams(entity) => self.mining_global_params.push(entity),
            Picked::MineableEntity(entity) => self.mineable_entities.push(entity),
            Picked::Provider(entity) => self.harvestable_provider_presets.push(entity),
            Picked::MultiConfig(entity) => self.sub_harvestable_multi_configs.push(entity),
            Picked::Mission(entity) => self.mission_kinds.push(entity),
            Picked::Loot(entity) => self.loot_kinds.push(entity),
            Picked::Crafter(entity) => self.crafters.push(entity),
            Picked::Crafted(pick, entity) => {
                self.crafters.push(entity);
                self.take(*pick, missing_categories);
            }
            Picked::Skipped(kind, guid) => {
                if kind == Kind::BlueprintCategories {
                    missing_categories.insert(guid);
                }
                self.skip(kind);
            }
            Picked::SkippedPaint => {
                self.skip(Kind::Items);
                self.skip(Kind::Paints);
            }
            Picked::Failed(failure) => self.failures.push(failure),
            Picked::Warned(warning) => self.warnings.push(warning),
        }
    }

    fn skip(&mut self, kind: Kind) {
        if let Some(skipped) = self.skipped.get_mut(kind.index()) {
            *skipped = skipped.saturating_add(1);
        }
    }

    fn crafting_skipped(crafting: &CraftingKinds<'f>) -> [usize; Kind::ALL.len()] {
        let mut skipped = [0; Kind::ALL.len()];
        for (kind, count) in &crafting.skipped {
            if let Some(slot) = skipped.get_mut(kind.index()) {
                *slot = *count;
            }
        }
        skipped
    }

    /// Stores the contracts, counts the entries left out as `skipped` and adds the failures.
    pub(crate) fn add_contracts(
        &mut self,
        contracts: Vec<ContractWithTitles<'f>>,
        skipped: usize,
        failures: Vec<CatalogReadFailure<'f>>,
    ) {
        (self.contracts, self.mission_titles) = contracts
            .into_iter()
            .map(|contract| (contract.row, contract.titles))
            .unzip();
        if let Some(slot) = self.skipped.get_mut(Kind::Contracts.index()) {
            *slot = skipped;
        }
        self.add_failures(failures);
    }

    /// Stores the location templates and the distinct spawn descriptions and location queries of
    /// the contracts, resolving the texts of the descriptions with `texts`, counts the templates
    /// left out as `skipped` and adds the failures.
    pub(crate) fn add_spawns(
        &mut self,
        templates: LocationTemplates<'f>,
        mut pools: SpawnPools<'f>,
        texts: &Texts<'f>,
    ) {
        self.ledger.merge(templates.ledger);
        self.mission_location_templates = templates.items;
        if let Some(slot) = self.skipped.get_mut(Kind::MissionLocationTemplates.index()) {
            *slot = templates.skipped;
        }
        pools.resolve_texts(texts);
        self.contract_ship_spawns = pools.ships.into_values().collect();
        self.contract_npc_spawns = pools.npcs.into_values().collect();
        self.contract_entity_spawns = pools.entities.into_values().collect();
        self.contract_location_queries = pools.locations.into_values().collect();
        self.add_failures(templates.failures);
    }

    /// Stores the contract templates, counts the ones left out as `skipped`, adds the failures and
    /// warns of the stored names `unknowns` counted.
    pub(crate) fn add_contract_templates(
        &mut self,
        templates: KindRead<'f, TemplateDefinition<'f>>,
        configurations: TemplateConfigurations<'f>,
        unknowns: Unknowns,
    ) {
        self.ledger.merge(templates.ledger);
        self.contract_templates = templates.items;
        self.template_configurations = configurations;
        if let Some(slot) = self.skipped.get_mut(Kind::ContractTemplates.index()) {
            *slot = templates.skipped;
        }
        self.add_failures(templates.failures);
        let (count, details) = unknowns.into_details();
        if count > 0 {
            self.warnings
                .push(CraftingWarning::UnknownTemplateValues { count, details });
        }
    }

    /// Stores the mission broker entries, counts the ones left out as `skipped` and adds the
    /// failures.
    pub(crate) fn add_broker_entries(
        &mut self,
        entries: KindRead<'f, BrokerProjection<'f>>,
        definitions: Vec<BrokerDefinition<'f>>,
    ) {
        self.ledger.merge(entries.ledger);
        self.broker_projections = entries.items;
        self.broker_definitions = definitions;
        if let Some(slot) = self.skipped.get_mut(Kind::MissionBrokerEntries.index()) {
            *slot = entries.skipped;
        }
        self.add_failures(entries.failures);
    }

    /// Adds `failures` and orders all of them by the text of the GUID, one per Record.
    pub(crate) fn add_failures(&mut self, failures: Vec<CatalogReadFailure<'f>>) {
        self.failures.extend(failures);
        self.order_failures();
    }

    fn order_failures(&mut self) {
        self.failures
            .sort_by_cached_key(|failure| CatalogGuid(failure.guid()).text());
        self.failures
            .dedup_by(|later, earlier| later.guid() == earlier.guid());
    }

    /// Returns the counts of every kind, in the order of [`Kind::ALL`].
    pub(crate) fn summary(&self, complete: bool) -> Vec<KindSummary> {
        Kind::ALL
            .into_iter()
            .map(|kind| KindSummary {
                kind,
                count: self.count(kind),
                skipped: self.skipped.get(kind.index()).copied().unwrap_or_default(),
                complete: complete || matches!(kind, Kind::Tags | Kind::ShopInventories),
            })
            .collect()
    }
}
