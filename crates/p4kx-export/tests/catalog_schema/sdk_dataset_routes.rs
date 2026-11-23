// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::with_ammo_definitions;
use p4kx_export::catalog_queries::faction_identities::with_faction_identities;
use p4kx_export::catalog_queries::weather::with_electrical_behaviors;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::Archive;

use super::{AMMUNITION, ELECTRICAL_BEHAVIORS, FACTION_IDENTITIES, Route, TestResult};

#[path = "sdk_actor_procedural_recoil_routes.rs"]
mod actor_procedural_recoil;
#[path = "sdk_archive_entry_routes.rs"]
mod archive_entries;
#[path = "sdk_archive_source_route.rs"]
pub(super) mod archive_source;
#[path = "sdk_reputation_definition_routes.rs"]
mod authored_reputation;
#[path = "sdk_blueprint_physical_routes.rs"]
mod blueprint_physical;
#[path = "sdk_blueprint_pool_routes.rs"]
mod blueprint_pools;
#[path = "sdk_blueprint_taxonomy_routes.rs"]
mod blueprint_taxonomy;
#[path = "sdk_cargo_routes.rs"]
mod cargo;
#[path = "sdk_client_profile_route.rs"]
mod client_profiles;
#[path = "sdk_climate_asset_route.rs"]
mod climate_assets;
#[path = "sdk_commodity_routes.rs"]
mod commodities;
#[path = "sdk_component_definition_routes.rs"]
mod component_definitions;
#[path = "sdk_equipment_component_parity.rs"]
mod component_parity;
#[path = "sdk_component_tool_routes.rs"]
mod component_tools;
#[path = "sdk_container_placement_routes.rs"]
mod container_placements;
#[path = "sdk_contract_scope_route.rs"]
mod contract_scopes;
#[path = "sdk_contract_routes.rs"]
mod contracts;
#[path = "sdk_countermeasure_emitter_route.rs"]
mod countermeasure_emitters;
#[path = "sdk_crafter_host_route.rs"]
pub(super) mod crafter_hosts;
#[path = "sdk_crafter_route.rs"]
pub(super) mod crafters;
#[path = "sdk_dismantle_process_route.rs"]
pub(super) mod dismantle;
#[path = "sdk_entity_animation_route.rs"]
pub(super) mod entity_animation;
#[path = "sdk_facility_route.rs"]
mod facilities;
#[path = "../it/catalog_facility_medbed_fixture.rs"]
mod facility_medbed_fixture;
#[path = "sdk_facility_placement_route.rs"]
mod facility_placements;
#[path = "sdk_faction_definition_route.rs"]
mod faction_definitions;
#[path = "sdk_faction_trigger_configuration_route.rs"]
mod faction_trigger_configuration;
#[path = "sdk_dataset_fixtures.rs"]
mod fixtures;
#[path = "sdk_flight_configuration_routes.rs"]
mod flight_configurations;
#[path = "sdk_fuel_routes.rs"]
mod fuel;
#[path = "sdk_generator_facet_routes.rs"]
mod generator_facets;
#[path = "sdk_hacking_routes.rs"]
mod hacking;
#[path = "sdk_interaction_route.rs"]
pub(super) mod interactions;
#[path = "sdk_inventory_container_route.rs"]
mod inventory_containers;
#[path = "sdk_item_configuration_routes.rs"]
mod item_configurations;
#[path = "sdk_item_durability_routes.rs"]
mod item_durability;
#[path = "sdk_item_recovery_routes.rs"]
mod item_recovery;
#[path = "sdk_jump_configuration_routes.rs"]
mod jump_configurations;
#[path = "sdk_law_route.rs"]
mod law;
#[path = "sdk_loadout_route.rs"]
mod loadout;
#[path = "sdk_location_vocabulary_route.rs"]
mod location_vocabulary;
#[path = "sdk_loot_routes.rs"]
mod loot;
#[path = "sdk_loot_generation_route.rs"]
mod loot_generation;
#[path = "sdk_medical_tier_route.rs"]
mod medical_tier;
#[path = "sdk_mineable_entity_route.rs"]
mod mineable_entity;
#[path = "sdk_mining_family_routes.rs"]
mod mining_families;
#[path = "sdk_mining_harvest_location_route.rs"]
mod mining_harvest_location;
#[path = "sdk_mission_board_route.rs"]
mod mission_board;
#[path = "sdk_mission_definition_routes.rs"]
mod mission_definitions;
#[path = "sdk_mission_locality_route.rs"]
mod mission_locality;
#[path = "sdk_mission_location_route.rs"]
mod mission_location;
#[path = "sdk_mission_type_route.rs"]
mod mission_type;
#[path = "sdk_npc_spawn_definition_routes.rs"]
mod npc_spawns;
#[path = "sdk_ordnance_routes.rs"]
mod ordnance;
#[path = "sdk_planet_asset_routes.rs"]
mod planet_assets;
#[path = "sdk_planet_audio_route.rs"]
mod planet_audio;
#[path = "sdk_planet_resource_route.rs"]
mod planet_resource;
#[path = "sdk_projectile_ammo_route.rs"]
mod projectile_ammo;
#[expect(
    dead_code,
    reason = "The schema witnesses select readable own-container projectile fixtures."
)]
#[path = "../it/catalog_weapon_projectile_ammo_fixture.rs"]
mod projectile_ammo_fixture;
#[path = "sdk_quality_routes.rs"]
mod quality;
#[path = "sdk_radar_options_route.rs"]
mod radar_options;
#[path = "sdk_reputation_routes.rs"]
mod reputation;
#[path = "../it/reputation_fixture/mod.rs"]
mod reputation_fixture;
#[path = "sdk_reputation_reward_route.rs"]
mod reputation_rewards;
#[path = "../it/reputation_sandbox_fixture.rs"]
mod reputation_sandbox_fixture;
#[path = "sdk_resource_group_routes.rs"]
mod resource_groups;
#[path = "sdk_resource_network_routes.rs"]
mod resource_network;
#[path = "sdk_resource_topology_routes.rs"]
mod resource_topology;
#[path = "sdk_ship_system_routes.rs"]
mod ship_systems;
#[path = "sdk_soil_routes.rs"]
mod soils;
#[path = "sdk_starmap_definition_routes.rs"]
pub(super) mod starmap_definitions;
#[path = "sdk_starmap_object_placement_route.rs"]
mod starmap_object_placement;
#[path = "sdk_tag_routes.rs"]
mod tag_routes;
#[path = "sdk_terrain_routes.rs"]
mod terrain;
#[path = "sdk_vehicle_armor_route.rs"]
mod vehicle_armor;
#[path = "sdk_weapon_animation_route.rs"]
mod weapon_animation;
#[path = "../it/catalog_weapon_operating_fixture.rs"]
mod weapon_operating_fixture;
#[path = "sdk_weapon_procedural_routes.rs"]
mod weapon_procedural;
#[path = "sdk_weapon_source_routes.rs"]
mod weapon_sources;
#[path = "sdk_worn_gear_routes.rs"]
mod worn_gear;

pub(super) fn contract_datasets() -> impl Iterator<Item = &'static str> {
    contracts::DATASETS.iter().copied()
}

pub(super) fn loot_datasets() -> impl Iterator<Item = &'static str> {
    loot::datasets()
}

pub(super) fn law_datasets() -> impl Iterator<Item = &'static str> {
    law::PATHS.into_iter()
}

pub(super) fn mission_datasets() -> impl Iterator<Item = &'static str> {
    use super::{MISSION_LOCATION_TEMPLATES as LOCATION_TEMPLATES, MISSION_TYPES};
    [
        LOCATION_TEMPLATES,
        MISSION_TYPES,
        mission_locality::DATASET,
        mission_board::DATASET,
    ]
    .into_iter()
    .chain(location_vocabulary::datasets())
    .chain(npc_spawns::datasets())
    .chain(mission_definitions::DATASETS)
}

pub(super) fn world_datasets() -> impl Iterator<Item = &'static str> {
    starmap_definitions::DATASETS
        .into_iter()
        .chain(soils::DATASETS)
        .chain(terrain::DATASETS)
        .chain([climate_assets::DATASET, ELECTRICAL_BEHAVIORS])
        .chain(planet_assets::DATASETS)
}

pub(super) fn read() -> TestResult<Vec<Route>> {
    let (_directory, archive) = fixtures::open()?;
    let [
        containers,
        location_instances,
        location_physics,
        loot_documents,
        unresolved,
    ] = container_placements::read(&archive, fixtures::ROOT)?;
    let independent_ammo = read_ammunition(&archive)?;
    let [projectile, reached_ammo, combat, magazine] = projectile_ammo::read()?;
    if independent_ammo.schema != reached_ammo.schema {
        return Err("independently admitted and reached Ammo rows have different schemas".into());
    }
    let mut routes = vec![
        reached_ammo,
        projectile,
        combat,
        magazine,
        weapon_sources::read_recoil(&archive)?,
        countermeasure_emitters::read()?,
        dismantle::read()?,
        crafters::read()?,
        crafter_hosts::read()?,
        archive_entries::read_language_policy()?,
        archive_entries::read_language_sources(&archive)?,
        location_vocabulary::read_entities(&archive)?,
        mission_location::read()?,
        mission_type::read()?,
        mission_locality::read()?,
        archive_entries::read_shop_inventory(&archive)?,
        archive_entries::read_shop_resolution()?,
        archive_entries::read_flare_library(&archive)?,
        read_faction_identities(&archive)?,
        faction_definitions::read()?,
        contract_scopes::read()?,
        faction_trigger_configuration::read()?,
        interactions::read()?,
        archive_entries::read_subsumption_event(&archive)?,
        archive_entries::read_radar_configuration(&archive)?,
        read_weather(&archive)?,
        containers,
        location_instances,
        location_physics,
        loot_documents,
        unresolved,
        starmap_object_placement::read()?,
    ];
    routes.extend(archive_source::read()?);
    append_domain_routes(&mut routes, &archive)?;
    Ok(routes)
}

fn append_domain_routes(routes: &mut Vec<Route>, archive: &Archive) -> TestResult<()> {
    routes.extend(blueprint_pools::read()?);
    routes.extend(blueprint_taxonomy::read()?);
    routes.extend(contracts::read()?);
    routes.extend(generator_facets::read()?);
    routes.extend(npc_spawns::read()?);
    routes.extend(blueprint_physical::read()?);
    routes.extend(component_definitions::read()?);
    routes.extend(flight_configurations::read()?);
    routes.extend(ship_systems::read()?);
    routes.extend(jump_configurations::read()?);
    routes.extend(worn_gear::read()?);
    routes.extend(ordnance::read()?);
    routes.push(vehicle_armor::read()?);
    routes.push(loot_generation::read()?);
    routes.push(entity_animation::read()?);
    routes.push(mission_board::read()?);
    routes.extend(mission_definitions::read()?);
    routes.extend(resource_groups::read()?);
    routes.extend(resource_topology::read()?);
    routes.extend(reputation::read()?);
    routes.push(reputation_rewards::read()?);
    routes.extend(resource_network::read()?);
    routes.extend(radar_options::read()?);
    routes.push(planet_audio::read()?);
    routes.push(archive_entries::read_particle_library(archive)?);
    routes.extend(weapon_animation::read()?);
    routes.extend(actor_procedural_recoil::read()?);
    routes.extend(weapon_procedural::read()?);
    routes.extend(fuel::read()?);
    routes.extend(hacking::read()?);
    routes.push(inventory_containers::read()?);
    routes.push(inventory_containers::read_configurations()?);
    routes.push(facilities::read()?);
    routes.extend(facility_placements::read()?);
    routes.push(client_profiles::read()?);
    routes.extend(cargo::read_globals(archive)?);
    routes.push(cargo::read_manifests()?);
    routes.extend(quality::read()?);
    item_configurations::append_item_routes(routes, archive)?;
    routes.push(tag_routes::entities(archive)?);
    routes.extend(tag_routes::declarations(archive)?);
    routes.push(tag_routes::paths(archive)?);
    routes.extend(commodities::read(archive)?);
    routes.extend(loot::read()?);
    routes.extend(law::read()?);
    routes.extend(location_vocabulary::read()?);
    routes.push(medical_tier::read()?);
    routes.push(mineable_entity::read()?);
    routes.extend(mining_families::read()?);
    routes.push(mining_harvest_location::read()?);
    routes.push(planet_resource::read()?);
    routes.extend(planet_assets::read()?);
    routes.push(climate_assets::read()?);
    routes.extend(component_tools::read()?);
    routes.extend(authored_reputation::read()?);
    routes.extend(weapon_sources::read()?);
    routes.extend(starmap_definitions::read()?);
    routes.extend(soils::read()?);
    routes.extend(terrain::read()?);
    Ok(())
}

pub(super) fn ammunition() -> TestResult<Route> {
    let (_directory, archive) = fixtures::open()?;
    read_ammunition(&archive)
}

fn read_ammunition(archive: &Archive) -> TestResult<Route> {
    with_ammo_definitions(
        archive,
        CatalogRecordSelection::RecordIndex(0),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            Route::queried_rows(
                AMMUNITION,
                snapshot.rows(),
                "independent Record selection available; complete dependency enumeration and installed parity open",
            )
        },
    )?
}

fn read_faction_identities(archive: &Archive) -> TestResult<Route> {
    with_faction_identities(
        archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            Route::queried_rows(
                FACTION_IDENTITIES,
                &snapshot.rows,
                "physical selection and synthetic Catalog parity available; installed parity open",
            )
        },
    )?
}

fn read_weather(archive: &Archive) -> TestResult<Route> {
    with_electrical_behaviors(archive, CatalogRecordSelection::All, None, |snapshot| {
        Route::queried_rows(
            ELECTRICAL_BEHAVIORS,
            snapshot.rows(),
            "synthetic canonical parity available; complete installed producer parity open",
        )
    })?
}

pub(super) const fn actor_procedural_recoil_datasets() -> [&'static str; 2] {
    actor_procedural_recoil::DATASETS
}

pub(super) const fn weapon_procedural_datasets() -> [&'static str; 2] {
    weapon_procedural::DATASETS
}

pub(super) const fn weapon_animation_datasets() -> [&'static str; 8] {
    weapon_animation::GRAPH_DATASETS
}

pub(super) const fn durability_datasets() -> [&'static str; 3] {
    item_durability::datasets()
}
