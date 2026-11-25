// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::CatalogLanguageSelection;
use p4kx_export::catalog_queries::flares::{FlareLibrarySelection, read_flare_libraries};
use p4kx_export::catalog_queries::localization::{catalog_language_policy, with_language_sources};
use p4kx_export::catalog_queries::particle_libraries::{
    ParticleLibrarySelection, read_particle_libraries,
};
use p4kx_export::catalog_queries::radar_audio::{RadarAudioSelection, read_radar_audio};
use p4kx_export::catalog_queries::shops::{
    ResolutionStatus, ShopInventorySelection, ShopRegistrySelection, ShopResolutionSelection,
    read_shop_inventories, read_shop_inventory_resolutions, read_shop_with_provenance_mode,
};
use p4kx_export::catalog_queries::subsumption_events::{
    SubsumptionEventSelection, read_subsumption_events,
};
use p4kx_export::{CatalogOptions, StarmapTraversalBudget, build_catalog};
use p4kx_extraction::{Archive, Language, find_entry};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use crate::sdk_full_proof as full_proof;

use super::super::{
    FLARE_LIBRARIES, LANGUAGE_POLICY, LANGUAGE_SOURCES, PARTICLE_LIBRARIES, RADAR_AUDIO,
    SHOP_INVENTORIES, SHOP_RESOLUTION, SHOPS, SUBSUMPTION_EVENTS,
};
use super::{Route, TestResult, fixtures};

pub(super) fn read_radar_configuration(archive: &Archive) -> TestResult<Route> {
    let snapshot = read_radar_audio(archive, RadarAudioSelection::All, None)?;
    Route::queried_rows(RADAR_AUDIO, &snapshot.rows, None)
}

pub(super) fn read_language_policy() -> TestResult<Route> {
    let policy = catalog_language_policy();
    assert!(
        policy.selects_available_supported_languages(),
        "the Catalog selects the supported languages available in the Archive"
    );
    assert!(
        !policy.uses_translation_fallback(),
        "the Catalog leaves missing translations absent"
    );
    let encoded = serde_json::to_value(&policy)?;
    assert_eq!(
        encoded,
        json!({"selection": "availableSupported", "fallback": "none"}),
        "both typed rules retain their canonical control vocabulary"
    );
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("language_policy.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            DataForgeBuilder::new().version(8).build(),
        ))
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            b"fixture=Original text\n",
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let built = build_catalog(
        &archive,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let full: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        full.pointer(LANGUAGE_POLICY),
        Some(&encoded),
        "the full normal Catalog writes the same archive-independent policy"
    );
    Route::queried_rows(LANGUAGE_POLICY, std::slice::from_ref(&policy), None)
}

pub(super) fn read_language_sources(archive: &Archive) -> TestResult<Route> {
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

pub(super) fn read_flare_library(archive: &Archive) -> TestResult<Route> {
    let selection = FlareLibrarySelection::ArchivePath(fixtures::FLARE_LIBRARY);
    let snapshot = read_flare_libraries(archive, selection, None)?;
    Route::queried_rows(
        FLARE_LIBRARIES,
        &snapshot.rows,
        "synthetic canonical parity and installed source witness available; installed producer parity open",
    )
}

pub(super) fn read_particle_library(archive: &Archive) -> TestResult<Route> {
    let snapshot = read_particle_libraries(archive, ParticleLibrarySelection::All, None)?;
    Route::queried_rows(
        PARTICLE_LIBRARIES,
        &snapshot.rows,
        "synthetic producer parity available; installed producer parity and memory evidence open",
    )
}

pub(super) fn read_subsumption_event(archive: &Archive) -> TestResult<Route> {
    let snapshot = read_subsumption_events(
        archive,
        SubsumptionEventSelection::ArchivePath(fixtures::SUBSUMPTION_FIXTURE),
        None,
    )?;
    Route::queried_rows(
        SUBSUMPTION_EVENTS,
        &snapshot.rows,
        "synthetic producer parity available; installed Event-ID and producer parity open",
    )
}

pub(super) fn read_shop_inventory(archive: &Archive) -> TestResult<Route> {
    let selection = ShopInventorySelection::ArchivePath(fixtures::OFFLINE_SHOP);
    let snapshot = read_shop_inventories(archive, selection, None)?;
    Route::queried_rows(
        SHOP_INVENTORIES,
        &snapshot.rows,
        "physical Entry and path query available; installed producer parity open",
    )
}

pub(super) fn read_shop_resolution() -> TestResult<Route> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("shop-resolution.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            r#"<BackupData><LayerBackups><LayerBackup><ShopBackups>
                <ShopBackup ID="00000000-0000-4000-8000-000000000001,00000000-0000-4000-8000-000000000002" SuperName="Fixture,Shop" />
                </ShopBackups></LayerBackup></LayerBackups></BackupData>"#,
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let snapshot = read_shop_inventory_resolutions(&archive, ShopResolutionSelection::All, None)?;
    let [row] = snapshot.rows.as_slice() else {
        return Err("the registry-only query must return its admitted shop".into());
    };
    assert_eq!(
        row.status().value(),
        Some(&ResolutionStatus::Unscanned),
        "no declaration source grants a physical target"
    );
    Route::queried_rows(SHOP_RESOLUTION, &snapshot.rows, None)
}

const REGISTRY: &str = "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml";
const INVENTORY_GUID: &str = "00000000-0000-4000-8000-000000000003";

pub(super) fn read_physical_shop() -> TestResult<Route> {
    let (_folder, archive) = physical_shop_fixture()?;
    let options = CatalogOptions::new(UNIX_EPOCH);
    let built = build_catalog(&archive, options.with_full_provenance())?;
    let document: Value = serde_json::from_slice(built.json())?;
    let rows = document
        .pointer(SHOPS)
        .and_then(Value::as_array)
        .ok_or("normal Shop rows")?;
    let [expected] = rows.as_slice() else {
        return Err("one normal Shop row".into());
    };
    let selected = registry_selection(&archive, &document, expected)?;
    let snapshot = read_shop_with_provenance_mode(
        &archive,
        selected,
        StarmapTraversalBudget::default(),
        full_proof::MODE,
        None,
    )?;
    assert!(
        snapshot.coverage.registry_complete && snapshot.coverage.names.is_none(),
        "the selected registry row was read and has no location dependency"
    );
    assert_eq!(
        snapshot.coverage.inventory_payloads, 1,
        "one chosen physical inventory body is required"
    );
    let row = snapshot.row.ok_or("one scoped Shop row")?;
    assert_eq!(
        row.registry.key.origin().contains_encrypted_source(),
        Some(false),
        "the key uses only the clear original registry"
    );
    let items = row
        .item_guids
        .value()
        .ok_or("the selected inventory items")?;
    let [item] = items.as_slice() else {
        return Err("one authored item GUID".into());
    };
    assert_eq!(
        item.value().map(String::as_str),
        Some(INVENTORY_GUID),
        "the typed row exposes the exact original item"
    );
    assert_eq!(
        item.origin().contains_encrypted_source(),
        Some(true),
        "the item value retains its own encrypted JSON origin"
    );
    assert_eq!(
        &serde_json::to_value(&row)?,
        expected,
        "the independently called physical route writes the exact normal Shop row"
    );
    Route::queried_rows(
        SHOPS,
        std::slice::from_ref(&row),
        "synthetic Normal row parity available; integrated installed producer parity open",
    )
}

fn physical_shop_fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("shop-row.p4k");
    let registry = r#"<BackupData><LayerBackups><LayerBackup PlatformName="FixtureSystem"><ShopBackups>
        <ShopBackup ID="00000000-0000-4000-8000-000000000001,00000000-0000-4000-8000-000000000002" SuperName="Fixture,Shop"/>
    </ShopBackups></LayerBackup></LayerBackups></BackupData>"#;
    let prefab = r#"<PrefabsLibrary><Prefab><Objects><Object Type="Entity" Id="{00004000-0000-0000-0200-000000000080}">
        <PropertiesDataCore><EntityComponentShop offlineInventoryJSON="Scripts/ShopInventories/shop.json"/></PropertiesDataCore>
    </Object></Objects></Prefab></PrefabsLibrary>"#;
    let inventory = json!({"Inventory": [{"ID": {"ID": [INVENTORY_GUID]}}]});
    ArchiveBuilder::v2()
        .entry(
            TestEntry::stored("Data/Game2.dcb", DataForgeBuilder::new().version(8).build())
                .with_encryption_field(0),
        )
        .entry(TestEntry::stored(REGISTRY, registry).with_encryption_field(0))
        .entry(TestEntry::stored("Data/Prefabs/Shops/shop.xml", prefab).with_encryption_field(0))
        .entry(
            TestEntry::stored(
                "Data/Scripts/ShopInventories/shop.json",
                serde_json::to_vec(&inventory)?,
            )
            .encrypted(),
        )
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    Ok((folder, archive))
}

fn registry_selection(
    archive: &Archive,
    catalog: &Value,
    row: &Value,
) -> TestResult<ShopRegistrySelection> {
    let registry = find_entry(archive, REGISTRY).ok_or("the physical shop registry Entry")?;
    let references = row
        .pointer("/key/sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("the original Shop key evidence")?;
    let uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("the normal producer source uses")?;
    let node = uses
        .iter()
        .filter(|used| used.get("id").is_some_and(|id| references.contains(id)))
        .find(|used| used.pointer("/location/field").and_then(Value::as_str) == Some("@ID"))
        .and_then(|used| used.pointer("/location/indices/0"))
        .and_then(Value::as_str)
        .ok_or("the original ShopBackup XML node coordinate")?;
    Ok(ShopRegistrySelection::new(
        u32::try_from(registry.index())?,
        node.parse()?,
    ))
}
