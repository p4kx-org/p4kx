// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::cargo_globals;
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
    with_location_entity_declarations,
};
use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntity, TestEntry, TestRecord, TestStruct,
    TestType, TestValue, entity_schema,
};

use super::{
    AMMUNITION, CARGO_LOADING_GLOBALS, FACTION_IDENTITIES, FLARE_LIBRARIES, ITEM_DEFINITIONS,
    LANGUAGE_SOURCES, LOCATION_ENTITIES, MANUFACTURERS, RADAR_AUDIO, Route, SHOP_COMMODITY_GLOBALS,
    SHOP_INVENTORIES, TestResult, WEAPON_RECOIL,
};

#[path = "sdk_blueprint_physical_routes.rs"]
mod blueprint_physical;
#[path = "sdk_blueprint_pool_routes.rs"]
mod blueprint_pools;
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
#[path = "sdk_flight_configuration_routes.rs"]
mod flight_configurations;
#[path = "sdk_fuel_routes.rs"]
mod fuel;
#[path = "sdk_loot_routes.rs"]
mod loot;
#[path = "sdk_loot_generation_route.rs"]
mod loot_generation;
#[path = "sdk_mineable_entity_route.rs"]
mod mineable_entity;
#[path = "sdk_planet_resource_route.rs"]
mod planet_resource;
#[path = "sdk_projectile_ammo_route.rs"]
mod projectile_ammo;
#[path = "sdk_quality_routes.rs"]
mod quality;
#[path = "sdk_resource_group_routes.rs"]
mod resource_groups;
#[path = "sdk_resource_network_routes.rs"]
mod resource_network;
#[path = "sdk_ship_system_routes.rs"]
mod ship_systems;
#[path = "sdk_vehicle_armor_route.rs"]
mod vehicle_armor;
#[path = "sdk_weapon_animation_route.rs"]
mod weapon_animation;
#[path = "sdk_worn_gear_routes.rs"]
mod worn_gear;

const ROOT: &str = r"Data\ObjectContainers\PU\system\fixture\fixturesystem.socpak";
const OFFLINE_SHOP: &str = r"Data\Scripts\ShopInventories\Inv_Fixture.json";
const FLARE_LIBRARY: &str = r"Data\Libs\Flares\fixture.xml";
const SUBSUMPTION_FIXTURE: &str = "Data/Libs/Subsumption/Events/synthetic.xml";
const TILE: &str = r"Data\ObjectContainers\PU\fixture\tile.socpak";
const ROOT_XML: &[u8] = br#"<ObjectContainer version="14" gridRadius="2"><PhysicsGrid/><ChildObjectContainers>
    <Child external="true" name="Data\ObjectContainers\PU\fixture\tile.socpak" pos="0,0,0" rot="1,0,0,0"/>
    <Child external="true" name="Data\ObjectContainers\PU\fixture\missing.socpak" pos="1,0,0" rot="1,0,0,0"/>
</ChildObjectContainers></ObjectContainer>"#;

pub(super) fn contract_datasets() -> impl Iterator<Item = &'static str> {
    contracts::DATASETS.iter().copied()
}

pub(super) fn loot_datasets() -> impl Iterator<Item = &'static str> {
    loot::datasets()
}

pub(super) fn read() -> TestResult<Vec<Route>> {
    let (_directory, archive) = fixture()?;
    let [containers, location_physics, unresolved] = container_placements::read(&archive, ROOT)?;
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
        weapon_animation::read()?,
        read_language_sources(&archive)?,
        read_location_entities(&archive)?,
        read_shop_inventory(&archive)?,
        read_flare_library(&archive)?,
        read_faction_identities(&archive)?,
        read_subsumption_event(&archive)?,
        read_radar_configuration(&archive)?,
        containers,
        location_physics,
        unresolved,
    ];
    routes.extend(blueprint_pools::read()?);
    routes.extend(contracts::read()?);
    routes.extend(blueprint_physical::read()?);
    routes.extend(component_definitions::read()?);
    routes.extend(flight_configurations::read()?);
    routes.extend(ship_systems::read()?);
    routes.extend(worn_gear::read()?);
    routes.push(vehicle_armor::read()?);
    routes.push(loot_generation::read()?);
    routes.extend(resource_groups::read()?);
    routes.extend(resource_network::read()?);
    routes.extend(fuel::read()?);
    routes.extend(read_cargo_globals(&archive)?);
    routes.extend(quality::read()?);
    routes.extend(read_entity_definitions(&archive)?);
    routes.extend(commodities::read(&archive)?);
    routes.extend(loot::read()?);
    routes.push(mineable_entity::read()?);
    routes.push(planet_resource::read()?);
    routes.extend(component_tools::read()?);
    Ok(routes)
}

pub(super) fn ammunition() -> TestResult<Route> {
    let (_directory, archive) = fixture()?;
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

fn read_location_entities(archive: &Archive) -> TestResult<Route> {
    with_location_entity_declarations(
        archive,
        CatalogRecordSelection::All,
        None,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| Route::queried_rows(LOCATION_ENTITIES, &snapshot.rows, None),
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

fn read_cargo_globals(archive: &Archive) -> TestResult<[Route; 2]> {
    const REMAINING: &str = "scoped installed parity proven; full Catalog parity remains open";
    let selection = CatalogRecordSelection::All;
    Ok([
        cargo_globals::with_cargo_loading_globals(archive, selection, None, |snapshot| {
            Route::queried_rows(CARGO_LOADING_GLOBALS, snapshot.rows(), REMAINING)
        })??,
        cargo_globals::with_shop_commodity_globals(archive, selection, None, |snapshot| {
            Route::queried_rows(SHOP_COMMODITY_GLOBALS, snapshot.rows(), REMAINING)
        })??,
    ])
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
    let selection = FlareLibrarySelection::ArchivePath(FLARE_LIBRARY);
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
        SubsumptionEventSelection::ArchivePath(SUBSUMPTION_FIXTURE),
        None,
    )?;
    Route::queried_rows(
        super::SUBSUMPTION_EVENTS,
        &snapshot.rows,
        "synthetic producer parity available; installed Event-ID and producer parity open",
    )
}

fn read_shop_inventory(archive: &Archive) -> TestResult<Route> {
    let selection = ShopInventorySelection::ArchivePath(OFFLINE_SHOP);
    let snapshot = read_shop_inventories(archive, selection, None)?;
    Route::queried_rows(
        SHOP_INVENTORIES,
        &snapshot.rows,
        "physical Entry and path query available; installed producer parity open",
    )
}

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("sdk_routes.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data\\Game2.dcb",
            fixture_database().build(),
        ))
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            b"fixture=Fixture\n",
        ))
        .entry(packed(ROOT, "fixturesystem.xml", ROOT_XML))
        .entry(packed(TILE, "tile.xml", b"<ObjectContainer/>"))
        .entry(TestEntry::stored(OFFLINE_SHOP, br#"{"Inventory":[]}"#))
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/RadarAudioData.xml",
            b"<RadarAudioData><DefaultZoom min='0'/></RadarAudioData>",
        ))
        .entry(TestEntry::stored(
            FLARE_LIBRARY,
            b"<LensFlareLibrary><FlareItem Name='fixture'/></LensFlareLibrary>",
        ))
        .entry(TestEntry::stored(
            SUBSUMPTION_FIXTURE,
            b"<Subsumption><Event ID='00000000-0000-0000-0000-000000000001'/></Subsumption>",
        ))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

fn fixture_database() -> DataForgeBuilder {
    let database = entity_schema(DataForgeBuilder::new().version(8))
        .structure(TestStruct::new("AmmoParams"))
        .record(TestRecord::new(
            1,
            "AmmoParams",
            "AmmoParams.FixtureBallistic",
            "libs/foundry/records/ammoparams/fixture_ballistic.xml",
        ))
        .structure(TestStruct::new("WeaponProceduralRecoilConfigDef"))
        .record(TestRecord::new(
            2,
            "WeaponProceduralRecoilConfigDef",
            "WeaponProceduralRecoilConfigDef.Fixture",
            "libs/foundry/records/fixture/recoil.xml",
        ))
        .structure(TestStruct::new("LocationEntityDeclaration"))
        .record(TestRecord::new(
            3,
            "LocationEntityDeclaration",
            "LocationEntityDeclaration.Fixture",
            "libs/foundry/records/fixture/location_entity.xml",
        ))
        .structure(TestStruct::new("GlobalCargoLoadingParams"))
        .record(TestRecord::new(
            4,
            "GlobalCargoLoadingParams",
            "GlobalCargoLoadingParams.Fixture",
            "libs/foundry/records/fixture/cargo_loading.xml",
        ))
        .structure(TestStruct::new("GlobalShopCommodityParams"))
        .record(TestRecord::new(
            5,
            "GlobalShopCommodityParams",
            "GlobalShopCommodityParams.Fixture",
            "libs/foundry/records/fixture/shop_commodity.xml",
        ))
        .structure(TestStruct::new("SCItemManufacturer").field("Code", TestType::String))
        .record(
            TestRecord::new(
                6,
                "SCItemManufacturer",
                "SCItemManufacturer.FixtureMaker",
                "libs/foundry/records/scitemmanufacturer/fixture.xml",
            )
            .value("Code", TestValue::text("FIX")),
        )
        .record(
            TestEntity::new(
                7,
                "FixtureTool",
                "libs/foundry/records/entities/scitem/fixture/tool.xml",
            )
            .attach_type("Misc")
            .record(),
        )
        .record(commodities::fixture_record());
    with_faction_identity(database)
}

fn with_faction_identity(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(TestStruct::new("FactionIdentity"))
        .record(TestRecord::new(
            9,
            "FactionIdentity",
            "FactionIdentity.Fixture",
            "libs/foundry/records/fixture/faction_identity.xml",
        ))
}

fn packed(path: &str, file: &str, xml: &[u8]) -> TestEntry {
    TestEntry::stored(
        path,
        TestContainer::new()
            .entry(TestEntry::stored(file, xml))
            .build(),
    )
}
