// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use crate::archive_key_use::ArchiveKeyUse;
use crate::build_derived::{add_contracts, add_loadouts, add_paints, add_tags};
use crate::build_harvest::add_harvest_locations;
use crate::build_loot::add_loot_containers;
use crate::build_starmap::add_starmap;
use crate::catalog_crafting::kinds;
use crate::catalog_definitions::CatalogDefinitions;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_projections::CatalogProjections;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::ammo_definition;
use crate::catalog_v3::catalog_independent_configurations::{self, IndependentConfigurations};
use crate::catalog_v3::inventory_container_definition::table as inventory_table;
use crate::catalog_v3::medical::medical_tier_config;
use crate::contract::{Catalog, Completeness, KindCompleteness, SCHEMA_VERSION};
use crate::crafting::read_with_progress;
use crate::mission_data::link_scenario_factions;
use crate::ship_store_slugs::StoreSlugs;
use crate::shops::add_shops;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, CatalogOptions, CatalogProvenance};

#[path = "build_catalog.rs"]
mod api;
pub use api::{build_catalog, build_catalog_from, write_catalog, write_catalog_from};
#[path = "catalog_refusal.rs"]
pub(crate) mod refusal;
use refusal::CatalogRefusal;

#[path = "original_catalog_inputs.rs"]
pub(crate) mod original_input;
use crate::catalog_read::original::EntryCapture;

#[path = "build_parts.rs"]
mod parts;
use parts::Parts;
#[path = "build_source.rs"]
mod metadata;
pub(crate) use metadata::source;

#[path = "build_scan.rs"]
mod scanning;

/// Builds the catalog and hands it to `emit`, which serialises it.
#[expect(
    clippy::too_many_lines,
    reason = "one step and one field per kind of the catalog"
)]
fn assemble<R>(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
    emit: impl FnOnce(&Catalog<'_>) -> Result<R, CatalogError>,
) -> Result<(Parts, R), CatalogError> {
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let forge = entry.open().map_err(CatalogError::DataForge)?;
    let sources = TextSources::read(archive)?;
    let texts = Texts::parse(&sources)?;
    let origins = CatalogOrigins::new(archive, entry, &sources)?;
    let input = original_input::OriginalCatalogInput::new(&forge, &origins);
    let projection = CatalogProjection::new(&texts, &input);
    let entry_capture = EntryCapture::for_input(&input);
    let crafting = kinds(
        read_with_progress(
            &forge,
            options.filter,
            options.cancel,
            options.progress,
            &projection,
        )?,
        &origins,
        options.filter,
    )?;
    let files = ArchiveFiles::new(archive);
    let entities = Arc::new(Entities::new(&forge));
    let (mut collected, mut placements) = scanning::scan(
        archive,
        &forge,
        &files,
        &entities,
        options,
        &projection,
        crafting,
    )?;
    let tags = add_tags(&forge, &input, options, &mut collected)?;
    collected.ledger.archive_inventory_problem = crate::export_archive_health::inventory(archive);
    let paint_summary = add_paints(&forge, &input, &mut collected, options.cancel)?;
    let loadout_summary = add_loadouts(archive, &entities, &mut collected, &input, options.cancel)?;
    let variants = collected.group_variants();
    let starmap_summary = add_starmap(&mut collected, placements.counts);
    add_harvest_locations(
        archive,
        &placements,
        &mut collected,
        options.cancel,
        &entry_capture,
    )?;
    add_loot_containers(
        archive,
        &placements,
        &mut collected,
        options.cancel,
        &entry_capture,
    )?;
    add_contracts(&forge, &projection, tags, options, &mut collected)?;
    collected.ledger.merge(link_scenario_factions(
        &forge,
        &mut collected.mission_kinds.scenarios,
        &input,
        options.cancel,
    )?);
    let shop_summary = add_shops(archive, &files, &mut collected, options, &entry_capture)?;
    let IndependentConfigurations {
        tables: independent,
        recovery_numbers,
        configuration_numbers,
    } = catalog_independent_configurations::read(&forge, &projection, options, &mut collected)?;
    let kinds = collected.summary(options.filter.is_none());
    let mut catalog_rows = crate::catalog_rows::snapshot(&mut collected, options.filter);
    catalog_rows.recovery_numbers = recovery_numbers;
    catalog_rows
        .configuration_numbers
        .extend(configuration_numbers);
    if collected
        .ledger
        .catalog_rows
        .replace(catalog_rows)
        .is_some()
    {
        collected.ledger.conflicting_catalog_rows = true;
    }
    let blueprints = crate::blueprint_catalog_rows::tables(
        std::mem::take(&mut collected.blueprints),
        &collected.missing_blueprint_categories,
        &collected.ledger.sources,
        &forge,
        &origins,
    )?;
    let instances = placements.take_instances();
    let (catalog_placements, resource_declarations) =
        collected.catalog_placements(archive, &forge, &input, instances, options.cancel)?;
    let health = crate::export_health_prepare::compile_captured(
        std::mem::take(&mut collected.ledger),
        &kinds,
        &collected.failures,
        &collected.warnings,
        options.publication_mode,
        &input,
        options.cancel,
    )?;
    if !health.permits_publication(options.publication_mode) {
        return Err(CatalogError::IncompleteExport(CatalogRefusal::new(
            Box::new(health),
            CatalogProvenance::new(origins.uses.freeze()?),
        )));
    }
    let key_use = ArchiveKeyUse::of(archive);
    let store_slugs = StoreSlugs::embedded()?;
    let resolved = store_slugs.resolve(
        collected
            .ships
            .iter()
            .map(|ship| (ship.guid.0, ship.class_name)),
        options.filter.is_some(),
    );
    let commodity_tables = crate::commodity::tables(&forge, collected.commodities);
    let group_tables = crate::catalog_v3::resource_group_catalog::tables(
        &forge,
        collected.resource_groups,
        collected.resource_group_members,
    );
    let reputation_tables = collected.mission_kinds.take_reputation_tables(&forge);
    let law_tables = collected.law.into_tables(&forge);
    let components = collected.item_components.into_tables(&forge);
    let patches = options.public_patches;
    let catalog = Catalog {
        schema_version: SCHEMA_VERSION,
        source: source(archive, options.exported_at, patches, key_use),
        completeness: Completeness::of_every_kind(KindCompleteness::for_filter(options.filter)),
        export_health: health.clone(),
        items: collected.items,
        ships: collected.ships,
        declarations: crate::catalog_v3::catalog_declarations::CatalogDeclarations {
            blueprint_category_lists: crate::catalog_v3::blueprint_category_list_read::table(
                &forge,
                collected.blueprint_category_lists,
            ),
            resource_group_members: group_tables.members,
            ammo_bindings: components.ammo_bindings,
            contract_scenario_requirements:
                crate::catalog_v3::mission_sources::contract_scenario_requirements::table(
                    &forge,
                    collected.contract_scenario_requirements,
                ),
            resource_network_links: resource_declarations.resource_network_links,
            room_associations: resource_declarations.room_associations,
            item_room_associations: resource_declarations.item_room_associations,
            shop_inventory_resolution: collected.shop_inventory_resolution,
            tag_roots: CatalogTable::new(&forge, &["TagDatabase"], collected.tags.roots),
            entity_tags: CatalogTable::new(
                &forge,
                &["EntityClassDefinition"],
                collected.tags.entity_tags,
            ),
        },
        placements: catalog_placements,
        definitions: CatalogDefinitions {
            blueprints: blueprints.definitions,
            shop_inventories: collected.shop_inventories,
            inventory_containers: inventory_table(&forge, collected.inventory_containers),
            medical_item_tier_configs: medical_tier_config::table(
                &forge,
                collected.medical_item_tier_configs,
            ),
            explosion_definitions: crate::catalog_v3::explosion_definition::table(
                &forge,
                collected.explosion_definitions,
            ),
            ammo_definitions: ammo_definition::table(&forge, collected.ammo_definitions),
            reputation_contexts: reputation_tables.contexts,
            faction_reputations: reputation_tables.factions.definitions,
            reputation_scopes: reputation_tables.scopes,
            reputation_standings: reputation_tables.standings,
            contract_generators: CatalogTable::new(
                &forge,
                &["ContractGenerator"],
                collected.contract_generators,
            ),
            mission_scenarios: collected.mission_kinds.take_scenario_definitions(&forge),
            contract_templates: CatalogTable::new(
                &forge,
                &["ContractTemplate"],
                collected.contract_templates,
            ),
            mission_broker_entries: CatalogTable::new(
                &forge,
                &["MissionBrokerEntry"],
                collected.broker_definitions,
            ),
            loot_pool_filters: CatalogTable::new(
                &forge,
                &["PoolFilterRecord"],
                collected.loot_kinds.filters,
            ),
            gameplay_properties: crate::catalog_v3::gameplay_property_catalog::table(
                &forge,
                collected.gameplay_properties,
            ),
            blueprint_categories: crate::blueprint_category::table(
                &forge,
                collected.blueprint_categories,
            ),
            quality_distributions: CatalogTable::new(
                &forge,
                &[
                    "CraftingQualityDistributionRecord",
                    "CraftingQualityLocationOverrideRecord",
                ],
                collected.quality.distributions,
            ),
            quality_quantizations: CatalogTable::new(
                &forge,
                &["CraftingQualityQuantizationRecord"],
                collected.quality.quantizations,
            ),
            resource_groups: group_tables.definitions,
            tags: CatalogTable::new(&forge, &["Tag"], collected.tags.definitions),
            resource_types: CatalogTable::new(&forge, &["ResourceType"], collected.resource_types),
            commodities: commodity_tables.definitions,
            manufacturers: crate::manufacturer::table(&forge, collected.manufacturers),
            factions: crate::catalog_v3::faction_definition::table(&forge, collected.factions),
            reputation_perks: reputation_tables.perks,
            reputation_trigger_records: reputation_tables.trigger_records,
            reputation_reward_amounts: reputation_tables.reward_amounts,
            infractions: law_tables.infractions,
            infraction_sets: law_tables.infraction_sets,
            jurisdictions: law_tables.jurisdictions,
            location_vocabulary: collected.location_vocabulary.into_tables(&forge),
            starmap: collected.starmap_definitions.into_tables(&forge),
        },
        projections: CatalogProjections {
            contracts: CatalogTable::new(&forge, &["ContractGenerator"], collected.contracts),
            mission_titles: CatalogTable::new(
                &forge,
                &["ContractGenerator"],
                collected.mission_titles,
            ),
            blueprint_presentation: blueprints.presentation,
            quality_quantization_resources: CatalogTable::new(
                &forge,
                &["CraftingQualityQuantizationRecord"],
                collected.quality.inverse,
            ),
            resource_group_parents: group_tables.parents,
            weapon_magazine_resolutions: components.magazine_resolutions,
            tag_paths: CatalogTable::new(&forge, &["Tag"], collected.tags.paths),
            commodity_presentation: commodity_tables.presentation,
        },
        configurations: crate::catalog_configurations::CatalogConfigurations {
            blueprint_tiers: blueprints.tiers,
            reputation_triggers: reputation_tables.factions.triggers,
            independent,
            durability: components.durability.health,
            degradation: components.durability.degradation,
            distortion: components.durability.distortion,
            explosives: components.explosives,
            hacking_chips: components.hacking_chips,
            ammo_containers: components.ammo_containers,
            weapon_actions: components.weapon_actions,
            quantum_drives: components.quantum_drives,
            shield_generators: components.shield_generators,
            entity_configurations: collected.entity_configurations.into_tables(&forge),
            local_contract_generators: collected.local_contract_generators,
            contract_templates: CatalogTable::new(
                &forge,
                &["ContractTemplate"],
                collected.template_configurations.templates,
            ),
            mission_broker_entries: CatalogTable::new(
                &forge,
                &["MissionBrokerEntry"],
                collected.broker_projections,
            ),
            mission_flows: CatalogTable::new(
                &forge,
                &["ContractTemplate", "MissionBrokerEntry"],
                collected.template_configurations.flows,
            ),
            mission_properties: CatalogTable::new(
                &forge,
                &["ContractTemplate", "MissionBrokerEntry"],
                collected.template_configurations.properties,
            ),
        },
        texts: projection.finish()?,
        provenance: CatalogProvenance::new(origins.uses.freeze()?),
        blueprint_pools: collected.blueprint_pools,
        dismantle_processes: collected.dismantle_processes,
        crafting_global: collected.crafting_global,
        paints: collected.paints,
        loadouts: collected.loadouts,
        mineable_elements: collected.mineable_elements,
        refining_processes: collected.refining_processes,
        mineable_compositions: collected.mineable_compositions,
        mining_global_params: collected.mining_global_params,
        harvestable_provider_presets: collected.harvestable_provider_presets,
        sub_harvestable_multi_configs: collected.sub_harvestable_multi_configs,
        harvestable_locations: collected.harvestable_locations,
        loot_containers: collected.loot_containers,
        shops: collected.shops,
        mission_types: collected.mission_kinds.types,
        mission_localities: collected.mission_kinds.localities,
        contract_scenarios: collected.mission_kinds.scenarios,
        loot_tables: collected.loot_kinds.tables,
        loot_archetypes: collected.loot_kinds.archetypes,
        legacy_loot_tables: collected.loot_kinds.legacy_tables,
        legacy_loot_archetypes: collected.loot_kinds.legacy_archetypes,
        mineable_entities: collected.mineable_entities,
        crafters: collected.crafters,
        cargo_manifests: collected.mission_kinds.manifests,
        mission_location_templates: collected.mission_location_templates,
        contract_ship_spawns: collected.contract_ship_spawns,
        contract_location_queries: collected.contract_location_queries,
        contract_npc_spawns: collected.contract_npc_spawns,
        contract_entity_spawns: collected.contract_entity_spawns,
        mission_property_sets: collected.mission_property_sets,
        ship_store_slugs: resolved.exported,
    };
    let output = emit(&catalog)?;
    Ok((
        Parts {
            export_health: health,
            provenance: catalog.provenance,
            kinds,
            texts: texts.summary(),
            notes: texts.notes().to_vec(),
            item_classes: collected.item_classes,
            paints: paint_summary,
            loadouts: loadout_summary,
            starmap: starmap_summary,
            starmap_report: Box::new(placements.report),
            shops: Box::new(shop_summary),
            store_slugs: Box::new(resolved.summary),
            playability: collected.playability,
            variants,
            quantizations: collected.quality.summary,
            warnings: collected.warnings,
            archive_key: key_use,
        },
        output,
    ))
}
