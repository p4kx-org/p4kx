// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    with_ammo_definitions, with_weapon_recoil_definitions,
};
use p4kx_export::catalog_queries::faction_identities::with_faction_identities;
use p4kx_export::catalog_queries::flares::{FlareLibrarySelection, read_flare_libraries};
use p4kx_export::catalog_queries::localization::with_language_sources;
use p4kx_export::catalog_queries::radar_audio::{RadarAudioSelection, read_radar_audio};
use p4kx_export::catalog_queries::shops::{ShopInventorySelection, read_shop_inventories};
use p4kx_export::catalog_queries::subsumption_events::{
    SubsumptionEventSelection, read_subsumption_events,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;

use super::{
    AMMUNITION, FACTION_IDENTITIES, FLARE_LIBRARIES, ITEM_DEFINITIONS, LANGUAGE_SOURCES,
    MANUFACTURERS, RADAR_AUDIO, Route, SHOP_INVENTORIES, TestResult, WEAPON_RECOIL,
};

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
#[path = "sdk_contract_routes.rs"]
mod contracts;
#[path = "sdk_countermeasure_emitter_route.rs"]
mod countermeasure_emitters;
#[path = "sdk_dismantle_process_route.rs"]
pub(super) mod dismantle;
#[path = "sdk_facility_route.rs"]
mod facilities;
#[path = "../it/catalog_facility_medbed_fixture.rs"]
mod facility_medbed_fixture;
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
#[path = "sdk_inventory_container_route.rs"]
mod inventory_containers;
#[path = "sdk_item_configuration_routes.rs"]
mod item_configurations;
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
#[path = "sdk_mission_locality_route.rs"]
mod mission_locality;
#[path = "sdk_mission_location_route.rs"]
mod mission_location;
#[path = "sdk_mission_type_route.rs"]
mod mission_type;
#[path = "sdk_npc_spawn_definition_routes.rs"]
mod npc_spawns;
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
#[path = "sdk_ship_system_routes.rs"]
mod ship_systems;
#[path = "sdk_starmap_definition_routes.rs"]
pub(super) mod starmap_definitions;
#[path = "sdk_tag_routes.rs"]
mod tag_routes;
#[path = "sdk_vehicle_armor_route.rs"]
mod vehicle_armor;
#[path = "sdk_weapon_animation_route.rs"]
mod weapon_animation;
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
    [LOCATION_TEMPLATES, MISSION_TYPES, mission_locality::DATASET]
        .into_iter()
        .chain(location_vocabulary::datasets())
        .chain(npc_spawns::datasets())
}

pub(super) fn read() -> TestResult<Vec<Route>> {
    let (_directory, archive) = fixtures::open()?;
    let [containers, location_physics, loot_documents, unresolved] =
        container_placements::read(&archive, fixtures::ROOT)?;
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
        read_weapon_recoil(&archive)?,
        countermeasure_emitters::read()?,
        dismantle::read()?,
        weapon_animation::read()?,
        read_language_sources(&archive)?,
        location_vocabulary::read_entities(&archive)?,
        mission_location::read()?,
        mission_type::read()?,
        mission_locality::read()?,
        read_shop_inventory(&archive)?,
        read_flare_library(&archive)?,
        read_faction_identities(&archive)?,
        faction_trigger_configuration::read()?,
        read_subsumption_event(&archive)?,
        read_radar_configuration(&archive)?,
        containers,
        location_physics,
        loot_documents,
        unresolved,
    ];
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
    routes.extend(worn_gear::read()?);
    routes.push(vehicle_armor::read()?);
    routes.push(loot_generation::read()?);
    routes.extend(resource_groups::read()?);
    routes.extend(reputation::read()?);
    routes.push(reputation_rewards::read()?);
    routes.extend(resource_network::read()?);
    routes.extend(fuel::read()?);
    routes.push(inventory_containers::read()?);
    routes.push(inventory_containers::read_configurations()?);
    routes.push(facilities::read()?);
    routes.push(client_profiles::read()?);
    routes.extend(cargo::read_globals(archive)?);
    routes.push(cargo::read_manifests()?);
    routes.extend(quality::read()?);
    routes.extend(read_entity_definitions(archive)?);
    routes.extend(item_configurations::read()?);
    routes.push(loadout::read()?);
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
    routes.push(planet_resource::read()?);
    routes.extend(component_tools::read()?);
    routes.extend(authored_reputation::read()?);
    routes.extend(weapon_sources::read()?);
    routes.extend(starmap_definitions::read()?);
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

fn read_radar_configuration(archive: &Archive) -> TestResult<Route> {
    let snapshot = read_radar_audio(archive, RadarAudioSelection::All, None)?;
    Route::queried_rows(
        RADAR_AUDIO,
        &snapshot.rows,
        "physical configuration query available; installed producer parity open",
    )
}

fn read_entity_definitions(archive: &Archive) -> TestResult<[Route; 2]> {
    const REMAINING: &str =
        "typed definitions and synthetic producer parity available; installed producer parity open";
    let session = CatalogQuerySession::open(archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let selection = CatalogRecordSelection::All;
    Ok([
        parsed.with_item_definitions(selection, None, |snapshot| {
            Route::queried_rows(ITEM_DEFINITIONS, snapshot.rows(), REMAINING)
        })??,
        parsed.with_manufacturers(selection, None, |snapshot| {
            Route::queried_rows(MANUFACTURERS, &snapshot.rows, REMAINING)
        })??,
    ])
}

fn read_language_sources(archive: &Archive) -> TestResult<Route> {
    with_language_sources(
        archive,
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| {
            if snapshot
                .sources
                .get("de")
                .and_then(|source| source.value())
                .is_none()
            {
                return Err("the language metadata query must return a present source".into());
            }
            Route::queried_value(
                LANGUAGE_SOURCES,
                &snapshot.sources,
                "selective language Entry metadata available; installed producer parity open",
            )
        },
    )?
}

fn read_weapon_recoil(archive: &Archive) -> TestResult<Route> {
    with_weapon_recoil_definitions(archive, CatalogRecordSelection::All, None, |snapshot| {
        Route::queried_rows(
            WEAPON_RECOIL,
            snapshot.rows(),
            "physical definition selection available; installed and complete producer parity open",
        )
    })?
}

fn read_flare_library(archive: &Archive) -> TestResult<Route> {
    let selection = FlareLibrarySelection::ArchivePath(fixtures::FLARE_LIBRARY);
    let snapshot = read_flare_libraries(archive, selection, None)?;
    Route::queried_rows(
        FLARE_LIBRARIES,
        &snapshot.rows,
        "synthetic canonical parity and installed source witness available; installed producer parity open",
    )
}

fn read_subsumption_event(archive: &Archive) -> TestResult<Route> {
    let snapshot = read_subsumption_events(
        archive,
        SubsumptionEventSelection::ArchivePath(fixtures::SUBSUMPTION_FIXTURE),
        None,
    )?;
    Route::queried_rows(
        super::SUBSUMPTION_EVENTS,
        &snapshot.rows,
        "synthetic producer parity available; installed Event-ID and producer parity open",
    )
}

fn read_shop_inventory(archive: &Archive) -> TestResult<Route> {
    let selection = ShopInventorySelection::ArchivePath(fixtures::OFFLINE_SHOP);
    let snapshot = read_shop_inventories(archive, selection, None)?;
    Route::queried_rows(
        SHOP_INVENTORIES,
        &snapshot.rows,
        "physical Entry and path query available; installed producer parity open",
    )
}
