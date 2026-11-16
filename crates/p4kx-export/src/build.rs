// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Archive, DataForgeEntry};

use crate::archive_key_use::ArchiveKeyUse;
use crate::build_derived::{add_contracts, add_loadouts, add_paints, add_tags, particle_library};
use crate::catalog_definitions::CatalogDefinitions;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_projections::CatalogProjections;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::catalog_independent_configurations;
use crate::catalog_v3::catalog_resource_network_read::resource_network_global;
use crate::catalog_v3::inventory_container_definition::table as inventory_table;
use crate::catalog_v3::medical::medical_tier_config;
use crate::catalog_v3::procedural_planet_audio;
use crate::catalog_v3::weapon_combat_base;
use crate::contract::{Catalog, Completeness, KindCompleteness, SCHEMA_VERSION};
use crate::mission_data::link_scenario_factions;
use crate::options::{CatalogRun, CatalogStage};
use crate::ship_store_slugs::StoreSlugs;
use crate::shops::add_shops;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, catalog_crafting};
#[path = "build_catalog.rs"]
pub(crate) mod api;
pub use api::{build_catalog, build_catalog_from, write_catalog, write_catalog_from};
#[path = "original_catalog_inputs.rs"]
pub(crate) mod original_input;
#[path = "build_parts.rs"]
mod parts;
#[path = "catalog_refusal.rs"]
pub(crate) mod refusal;
use parts::Parts;
use refusal::PendingRefusal;
#[path = "build_source.rs"]
mod metadata;
pub(crate) use metadata::source;
#[path = "build_scan.rs"]
mod scanning;
#[expect(
    clippy::too_many_lines,
    reason = "one step and one field per kind of the catalog"
)]
fn assemble<R>(
    archive: &Archive,
    entry: &DataForgeEntry,
    run: CatalogRun<'_, '_>,
    emit: impl FnOnce(&Catalog<'_>) -> Result<R, CatalogError>,
) -> Result<ControlFlow<PendingRefusal, (Parts, R)>, CatalogError> {
    let options = run.options;
    run.check_cancelled()?;
    run.begin(CatalogStage::ReadInputs);
    run.begin(CatalogStage::OpenDataForge);
    let forge = entry.open().map_err(CatalogError::DataForge)?;
    run.begin(CatalogStage::ReadLocalization);
    let sources = TextSources::read(archive)?;
    run.begin(CatalogStage::IndexLocalization);
    let texts = Texts::parse(&sources)?;
    run.begin(CatalogStage::InitializeOrigins);
    let origins = CatalogOrigins::new(archive, entry, &sources)?;
    let input = original_input::OriginalCatalogInput::new(&forge, &origins);
    let text_registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &text_registry);
    let entry_capture = EntryCapture::for_input(&input);
    run.begin(CatalogStage::Crafting);
    let (crafting, blueprint_definitions) = catalog_crafting::read(&forge, options, &projection)?;
    let files = ArchiveFiles::new(archive);
    let entities = Arc::new(Entities::new(&forge));
    let (mut collected, mut placements) = scanning::scan(
        archive,
        &input,
        &files,
        &entities,
        run,
        &projection,
        crafting,
    )?;
    let (tags, paint_tags) = add_tags(&forge, &input, options, &mut collected)?;
    let paint_summary = add_paints(paint_tags, &mut collected, options.cancel)?;
    let ship_variant_keys = collected.take_ship_variant_keys();
    run.begin(CatalogStage::ResolveLoadouts);
    let loadout_summary = add_loadouts(&files, &entities, &mut collected, &input, options.cancel)?;
    let variants = collected.group_variants(ship_variant_keys);
    let starmap_summary = scanning::project_locations(
        archive,
        &mut placements,
        &mut collected,
        &entry_capture,
        run,
    )?;
    run.begin(CatalogStage::ProjectMissions);
    add_contracts(&forge, &projection, tags, run, &mut collected)?;
    run.begin(CatalogStage::LinkMissionFactions);
    collected.ledger.merge(link_scenario_factions(
        &forge,
        &mut collected.mission_kinds.scenarios,
        &input,
        options.cancel,
    )?);
    run.begin(CatalogStage::ProjectConfigurations);
    run.begin(CatalogStage::ProjectShops);
    let shop_summary = add_shops(archive, &files, &mut collected, options, &entry_capture)?;
    run.begin(CatalogStage::ResolveJumpOperators);
    let jump_owners = placements.join_jump_operators(&entities, &input, options.cancel)?;
    let jump_connections = CatalogTable::empty(placements.jump_connection_availability());
    run.begin(CatalogStage::ReadIndependentConfigurations);
    let independent = catalog_independent_configurations::read(
        &forge,
        &projection,
        options,
        &mut collected,
        &jump_owners,
    )?;
    run.begin(CatalogStage::ProjectParticleLibraries);
    particle_library::collect(archive, &entry_capture, options, &mut collected)?;
    run.begin(CatalogStage::ProjectWeaponAnimation);
    let weapon_animation =
        parts::capture_weapon_animation(archive, &entities, &input, options, &mut collected)?;
    drop(entities);
    run.begin(CatalogStage::FinalizeProjections);
    let kinds = collected.summary(options.filter.is_none());
    parts::capture_rows(
        &mut collected,
        options.filter,
        independent.recovery_numbers,
        independent.configuration_numbers,
    );
    run.begin(CatalogStage::ProjectBlueprintTables);
    let blueprints = parts::blueprint_tables(&mut collected, &forge, &origins)?;
    let (catalog_placements, resource_declarations) = collected.catalog_placements(
        archive,
        &forge,
        &input,
        &mut placements,
        &independent.tables.flight,
        run,
    )?;
    let health = parts::export_health(&mut collected, &kinds, &input, run)?;
    if !health.permits_publication(options.publication_mode) {
        let refusal = PendingRefusal::new(health, origins.uses.clone());
        return Ok(ControlFlow::Break(refusal));
    }
    drop(collected.failures);
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
    let quality = collected.quality.take_tables(&forge);
    let law_tables = collected.law.into_tables(&forge);
    let components = collected.item_components.into_tables(&forge)?;
    let weapon_tables = weapon_combat_base::CatalogWeaponTables::build(
        &forge,
        &input,
        &components,
        collected.ammo_definitions,
        options.cancel,
    )?;
    let finished_texts = projection.finish()?;
    let (text_summary, text_notes) = texts.into_summary();
    let catalog = Catalog {
        schema_version: SCHEMA_VERSION,
        source: source(
            archive,
            options.exported_at,
            options.public_patches,
            key_use,
        ),
        completeness: Completeness::of_every_kind(KindCompleteness::for_filter(options.filter)),
        export_health: health,
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
            items: parts::item_definitions(&forge, collected.item_definitions),
            blueprints: blueprint_definitions,
            shop_inventories: collected.shop_inventories,
            particle_libraries: collected.particle_libraries,
            planet_tech: collected.planet_tech,
            inventory_containers: inventory_table(&forge, collected.inventory_containers),
            medical_item_tier_configs: medical_tier_config::table(
                &forge,
                collected.medical_item_tier_configs,
            ),
            procedural_planet_audio: procedural_planet_audio::table(
                &forge,
                collected.procedural_planet_audio,
            ),
            resource_network_globals: resource_network_global::table(
                &forge,
                collected.resource_network_globals,
            ),
            explosion_definitions: crate::catalog_v3::explosion_definition::table(
                &forge,
                collected.explosion_definitions,
            ),
            ammo_definitions: weapon_tables.ammunition,
            weapon_action_recoil_definitions: independent.weapon_action_recoil_definitions,
            weapon_animation_sources: weapon_animation.sources,
            weapon_procedural: independent.weapon_procedural,
            damage_resistance_macros: independent.damage_resistance_macros,
            turret_health_modifiers: independent.turret_health_modifiers,
            consumables: independent.consumable_definitions,
            mining_laser_globals: independent.mining_laser_globals,
            reputation_contexts: reputation_tables.contexts,
            faction_reputations: reputation_tables.factions.definitions,
            reputation_scopes: reputation_tables.scopes,
            reputation_standings: reputation_tables.standings,
            contract_generators: collected.contract_generators,
            mission_scenarios: collected.mission_kinds.take_scenario_definitions(&forge),
            mission_module_declarations: collected.mission_kinds.take_module_declarations(&forge),
            mission_items: collected.mission_kinds.take_items(&forge),
            contract_templates: collected.contract_templates.table,
            mission_broker_entries: collected.broker_definitions.table,
            loot: collected.loot_kinds.definitions.tables(&forge),
            gameplay_properties: crate::catalog_v3::gameplay_property_catalog::table(
                &forge,
                collected.gameplay_properties,
            ),
            blueprint_categories: crate::blueprint_category::table(
                &forge,
                collected.blueprint_categories,
            ),
            quality_distributions: quality.distributions,
            quality_quantizations: quality.quantizations,
            resource_groups: group_tables.definitions,
            tags: CatalogTable::new(&forge, &["Tag"], collected.tags.definitions),
            resource_types: CatalogTable::new(&forge, &["ResourceType"], collected.resource_types),
            commodities: commodity_tables.definitions,
            manufacturers: crate::manufacturer::table(&forge, collected.manufacturers),
            factions: crate::catalog_v3::faction_definition::table(&forge, collected.factions),
            faction_identities: parts::faction_identities(&forge, collected.faction_identities),
            reputation_perks: reputation_tables.perks,
            reputation_trigger_records: reputation_tables.trigger_records,
            reputation_reward_amounts: reputation_tables.reward_amounts,
            law: law_tables,
            location_vocabulary: collected.location_vocabulary.into_tables(&forge),
            npc_spawn_definitions: collected.npc_spawn_definitions.into_tables(&forge),
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
            quality_quantization_resources: quality.resources,
            resource_group_parents: group_tables.parents,
            weapon_magazine_resolutions: components.magazine_resolutions,
            weapon_combat_bases: weapon_tables.combat,
            weapon_player_animation_candidates: weapon_animation.candidates,
            weapon_projectile_ammo_selections: components.projectile_ammo_selections,
            tag_paths: CatalogTable::new(&forge, &["Tag"], collected.tags.paths),
            commodity_presentation: commodity_tables.presentation,
            jump_connections,
        },
        configurations: crate::catalog_configurations::CatalogConfigurations {
            blueprint_tiers: blueprints.tiers,
            reputation_triggers: reputation_tables.factions.triggers,
            independent: independent.tables,
            durability: collected.item_health,
            degradation: components.durability.degradation,
            distortion: components.durability.distortion,
            emp: components.emp,
            explosives: components.explosives,
            hacking_chips: components.hacking_chips,
            ammo_containers: components.ammo_containers,
            countermeasure_emitters: weapon_tables.emitters,
            weapon_actions: components.weapon_actions,
            missile_configurations: components.missile_configurations,
            jump_drives: components.jump_drives,
            cargo_globals: collected.cargo_globals.into_tables(&forge),
            quantum_drives: components.quantum_drives,
            quantum_interdiction: components.quantum_interdiction,
            shield_generators: components.shield_generators,
            entity_configurations: collected.entity_configurations.into_tables(&forge)?,
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
        texts: finished_texts.texts,
        provenance: parts::freeze_provenance(&origins.uses, run)?,
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
    Ok(ControlFlow::Continue((
        Parts {
            export_health: catalog.export_health,
            provenance: catalog.provenance,
            kinds,
            texts: text_summary,
            text_kind_coverage: finished_texts.kind_coverage,
            notes: text_notes,
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
    )))
}
