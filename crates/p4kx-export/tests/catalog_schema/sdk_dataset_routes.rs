// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::StarmapTraversalBudget;
use p4kx_export::catalog_queries::equipment::{
    with_ammo_definitions, with_weapon_recoil_definitions,
};
use p4kx_export::catalog_queries::localization::with_language_sources;
use p4kx_export::catalog_queries::shops::{ShopInventorySelection, read_shop_inventories};
use p4kx_export::catalog_queries::starmap::{
    ContainerPlacementSelection, read_container_placements,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntry, TestRecord, TestStruct,
};

use super::{
    AMMUNITION, CONTAINERS, LANGUAGE_SOURCES, Route, SHOP_INVENTORIES, TestResult, UNRESOLVED,
    WEAPON_RECOIL,
};

#[path = "sdk_projectile_ammo_route.rs"]
mod projectile_ammo;
#[path = "sdk_weapon_animation_route.rs"]
mod weapon_animation;

const ROOT: &str = r"Data\ObjectContainers\PU\system\fixture\fixturesystem.socpak";
const OFFLINE_SHOP: &str = r"Data\Scripts\ShopInventories\Inv_Fixture.json";
const TILE: &str = r"Data\ObjectContainers\PU\fixture\tile.socpak";
const ROOT_XML: &[u8] = br#"<ObjectContainer version="14"><ChildObjectContainers>
    <Child external="true" name="Data\ObjectContainers\PU\fixture\tile.socpak" pos="0,0,0" rot="1,0,0,0"/>
    <Child external="true" name="Data\ObjectContainers\PU\fixture\missing.socpak" pos="1,0,0" rot="1,0,0,0"/>
</ChildObjectContainers></ObjectContainer>"#;

pub(super) fn read() -> TestResult<Vec<Route>> {
    let (_directory, archive) = fixture()?;
    let root = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == ROOT)
        .ok_or("original root archive Entry")?;
    let starmap = read_container_placements(
        &archive,
        ContainerPlacementSelection::SystemRoot(u32::try_from(root.index())?),
        StarmapTraversalBudget::new(),
        None,
    )?;
    let independent_ammo = read_ammunition(&archive)?;
    let [projectile, reached_ammo, combat, magazine] = projectile_ammo::read()?;
    if independent_ammo.schema != reached_ammo.schema {
        return Err("independently admitted and reached Ammo rows have different schemas".into());
    }
    Ok(vec![
        reached_ammo,
        projectile,
        combat,
        magazine,
        read_weapon_recoil(&archive)?,
        weapon_animation::read()?,
        read_language_sources(&archive)?,
        read_shop_inventory(&archive)?,
        Route::queried_rows(
            CONTAINERS,
            &starmap.container_instances,
            "system-root and occurrence query available; root enumeration and installed parity open",
        )?,
        Route::queried_rows(
            UNRESOLVED,
            &starmap.unresolved_container_links,
            "system-root and occurrence query available; root enumeration and installed parity open",
        )?,
    ])
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
    let database = DataForgeBuilder::new()
        .version(8)
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
        ));
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("sdk_routes.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database.build()))
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            b"fixture=Fixture\n",
        ))
        .entry(packed(ROOT, "fixturesystem.xml", ROOT_XML))
        .entry(packed(TILE, "tile.xml", b"<ObjectContainer/>"))
        .entry(TestEntry::stored(OFFLINE_SHOP, br#"{"Inventory":[]}"#))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

fn packed(path: &str, file: &str, xml: &[u8]) -> TestEntry {
    TestEntry::stored(
        path,
        TestContainer::new()
            .entry(TestEntry::stored(file, xml))
            .build(),
    )
}
