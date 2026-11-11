// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use crate::archive_key_use::ArchiveKeyUse;
use crate::build_derived::{add_contracts, add_loadouts, add_paints};
use crate::build_harvest::add_harvest_locations;
use crate::build_loot::add_loot_containers;
use crate::build_starmap::add_starmap;
use crate::catalog_crafting::kinds;
use crate::catalog_definitions::CatalogDefinitions;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_projections::CatalogProjections;
use crate::catalog_table::CatalogTable;
use crate::contract::{Catalog, Completeness, KindCompleteness, SCHEMA_VERSION};
use crate::crafting::read_with_progress;
use crate::mission_data::link_scenario_factions;
use crate::recovery::{Recovery, read_recovery};
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
use original_input::OriginalCatalogInput;

#[path = "build_parts.rs"]
mod parts;
use parts::Parts;

#[path = "build_source.rs"]
mod metadata;
pub(crate) use metadata::source;

#[path = "build_scan.rs"]
mod scanning;
use scanning::scan;

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
    let input = OriginalCatalogInput::new(&forge, &origins);
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
    let (mut collected, placements) = scan(
        archive,
        &forge,
        &files,
        &entities,
        options,
        &projection,
        crafting,
    )?;
    let need_mission_tags =
        crate::mission_index::MissionIndex::needs_index(&forge, options.filter, options.cancel)?;
    let mut tags =
        crate::catalog_v3::tag_catalog::read(&forge, &input, options.cancel, need_mission_tags)?;
    tags.rows.entity_tags = std::mem::take(&mut collected.tags.entity_tags);
    collected.tags = tags.rows;
    collected.ledger.merge(tags.ledger);
    collected.add_failures(tags.failures);
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
    add_contracts(
        &forge,
        &projection,
        (&tags.context, tags.dependencies),
        options,
        &mut collected,
    )?;
    collected.ledger.merge(link_scenario_factions(
        &forge,
        &mut collected.mission_kinds.scenarios,
        &input,
        options.cancel,
    )?);
    let shop_summary = add_shops(archive, &files, &mut collected, options, &entry_capture)?;
    let Recovery {
        rules,
        failures,
        ledger,
    } = read_recovery(&forge, &input, options.filter, options.cancel)?;
    collected.ledger.merge(ledger);
    collected.add_failures(failures);
    let kinds = collected.summary(options.filter.is_none());
    let catalog_rows = crate::catalog_rows::snapshot(&collected, options.filter);
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
    let (commodity_definitions, commodity_presentation) =
        crate::commodity::split(collected.commodities);
    let commodities = CatalogTable::new(
        &forge,
        &["EntityClassDefinition", "ResourceType"],
        commodity_definitions,
    );
    let commodity_presentation = CatalogTable {
        availability: commodities.availability,
        rows: commodity_presentation,
    };
    let group_tables = crate::catalog_v3::resource_group_catalog::tables(
        &forge,
        collected.resource_groups,
        collected.resource_group_members,
    );
    let durability = crate::catalog_v3::catalog_durability::tables(&forge, collected.durability);
    let catalog = Catalog {
        schema_version: SCHEMA_VERSION,
        source: source(
            archive,
            options.exported_at,
            options.public_patches,
            key_use,
        ),
        completeness: Completeness::of_every_kind(completeness_of(options)),
        export_health: health.clone(),
        items: collected.items,
        ships: collected.ships,
        declarations: crate::catalog_v3::catalog_declarations::CatalogDeclarations {
            blueprint_category_lists: crate::catalog_v3::blueprint_category_list_read::table(
                &forge,
                collected.blueprint_category_lists,
            ),
            resource_group_members: group_tables.members,
            shop_inventory_resolution: collected.shop_inventory_resolution,
            tag_roots: CatalogTable::new(&forge, &["TagDatabase"], collected.tags.roots),
            entity_tags: CatalogTable::new(
                &forge,
                &["EntityClassDefinition"],
                collected.tags.entity_tags,
            ),
        },
        definitions: CatalogDefinitions {
            blueprints: blueprints.definitions,
            shop_inventories: collected.shop_inventories,
            contract_generators: CatalogTable::new(
                &forge,
                &["ContractGenerator"],
                collected.contract_generators,
            ),
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
            commodities,
            manufacturers: crate::manufacturer::table(&forge, collected.manufacturers),
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
            tag_paths: CatalogTable::new(&forge, &["Tag"], collected.tags.paths),
            commodity_presentation,
        },
        configurations: crate::catalog_configurations::CatalogConfigurations {
            blueprint_tiers: blueprints.tiers,
            durability: durability.health,
            degradation: durability.degradation,
            distortion: durability.distortion,
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
        starmap_objects: collected.starmap_objects,
        mineable_elements: collected.mineable_elements,
        refining_processes: collected.refining_processes,
        mineable_compositions: collected.mineable_compositions,
        mining_global_params: collected.mining_global_params,
        harvestable_provider_presets: collected.harvestable_provider_presets,
        sub_harvestable_multi_configs: collected.sub_harvestable_multi_configs,
        harvestable_locations: collected.harvestable_locations,
        loot_containers: collected.loot_containers,
        shops: collected.shops,
        faction_reputations: collected.mission_kinds.factions,
        reputation_scopes: collected.mission_kinds.scopes,
        reputation_standings: collected.mission_kinds.standings,
        reputation_contexts: collected.mission_kinds.contexts,
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
        item_recovery: rules,
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

fn completeness_of<'o>(options: &'o CatalogOptions<'_>) -> KindCompleteness<'o> {
    options.filter.map_or(KindCompleteness::Complete, |filter| {
        KindCompleteness::Partial {
            filter: filter.as_str(),
        }
    })
}
