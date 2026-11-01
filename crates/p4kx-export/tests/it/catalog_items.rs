// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Items and ships in the Basetool catalog: selection, fields and item class, through the public
//! API.

use crate::catalog_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use catalog_fixture::{Maker, english, write_archive, write_archive_with};
use p4kx_export::{
    BuiltCatalog, CatalogOptions, Filter, ItemClass, ItemClassSummary, Kind, build_catalog,
};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);

fn options() -> CatalogOptions<'static> {
    CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
}

fn catalog_with(options: &CatalogOptions<'_>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        write_archive(dir.path(), &[english()])?,
        Some(p4kx_testkit::test_key()),
    )?;
    let built = build_catalog(&archive, options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn catalog_of(maker: Maker) -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        write_archive_with(dir.path(), &[english()], maker)?,
        Some(p4kx_testkit::test_key()),
    )?;
    let built = build_catalog(&archive, &options())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn manufacturer_guids<'c>(catalog: &'c Value, kind: &str) -> Vec<&'c Value> {
    catalog[kind]
        .as_array()
        .map(|list| {
            list.iter()
                .filter(|entry| entry["className"] != "Counter_Shop")
                .map(|entry| &entry["manufacturerGuid"])
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_manufacturer_given_by_reference_is_its_guid() -> TestResult {
    let catalog = catalog_of(Maker::Reference)?;
    for kind in ["items", "ships"] {
        let guids = manufacturer_guids(&catalog, kind);
        assert!(!guids.is_empty(), "{kind} present");
        assert!(
            guids
                .iter()
                .all(|guid| **guid == "10000000-0000-4000-8000-000000000001"),
            "{kind}: {guids:?}"
        );
    }
    Ok(())
}

#[test]
fn a_manufacturer_given_by_a_strong_or_weak_pointer_is_null() -> TestResult {
    for maker in [Maker::StrongPointer, Maker::WeakPointer] {
        let catalog = catalog_of(maker)?;
        for kind in ["items", "ships"] {
            let guids = manufacturer_guids(&catalog, kind);
            assert!(!guids.is_empty(), "{kind} present");
            assert!(guids.iter().all(|guid| guid.is_null()), "{kind}: {guids:?}");
        }
    }
    Ok(())
}

fn class_names<'c>(catalog: &'c Value, kind: &str) -> Vec<&'c str> {
    catalog[kind]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|entry| entry["className"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

fn entity<'c>(catalog: &'c Value, kind: &str, class: &str) -> Result<&'c Value, String> {
    catalog[kind]
        .as_array()
        .and_then(|list| list.iter().find(|entry| entry["className"] == class))
        .ok_or_else(|| format!("{kind} {class}"))
}

#[test]
fn items_are_entity_class_definitions_under_scitem_and_legacyitems_ordered_by_guid() -> TestResult {
    let (built, catalog) = catalog_with(&options())?;
    assert_eq!(
        class_names(&catalog, "items"),
        [
            "BEHR_Rifle_Ballistic_01",
            "POWR_ACAS_S01",
            "Hat_01",
            "Old_Box",
            "Counter_Shop"
        ],
        "by prefix and type, without leftovers, by GUID"
    );
    let kind = built.summary().kind(Kind::Items);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((5, 1, true)),
        "the test rifle is skipped"
    );
    Ok(())
}

#[test]
fn an_item_carries_its_definition_mass_display_type_and_class() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    let mut actual = entity(&catalog, "items", "POWR_ACAS_S01")?.clone();
    if let Some(fields) = actual.as_object_mut() {
        fields.remove("variantOf");
    }
    assert_eq!(
        &actual,
        &json!({
            "guid": "30000000-0000-4000-8000-000000000001",
            "className": "POWR_ACAS_S01",
            "recordPath": "libs/foundry/records/entities/scitem/ships/powerplant/powr_acas_s01",
            "name": {"key": "@item_NamePOWR_ACAS_S01", "en": "Lumen", "de": null},
            "description": {
                "key": "@item_DescPOWR_ACAS_S01",
                "en": "Item Type: Power Plant\nClass:\u{a0}Military \n\nSteady power.",
                "de": null
            },
            "type": "PowerPlant",
            "subType": "Power",
            "size": 1,
            "grade": 3,
            "mass": 125.5,
            "manufacturerGuid": "10000000-0000-4000-8000-000000000001",
            "tags": ["flightReady", "$Small"],
            "displayType": {"key": "@item_DisplayType_PowerPlant", "en": "Power Plant", "de": null},
            "itemClass": "Military",
            "playability": null,
            "stats": {
                "weapon": null,
                "shield": null,
                "quantumDrive": null,
                "powerPlant": {"powerOutput": null, "unit": null},
                "cooler": null,
                "missile": null,
                "armor": null,
                "clothing": null,
                "radar": null,
                "tractorBeam": null,
                "weaponModifier": null,
                "miningLaser": null,
                "miningModifier": null,
                "health": null,
                "resourceStates": []
            },
        }),
        "the item fields"
    );
    Ok(())
}

#[test]
fn an_item_without_an_item_definition_keeps_its_identity_and_null_fields() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    let counter = entity(&catalog, "items", "Counter_Shop")?;
    for field in [
        "type",
        "subType",
        "size",
        "grade",
        "mass",
        "manufacturerGuid",
        "itemClass",
    ] {
        assert_eq!(counter[field], Value::Null, "{field} is null");
    }
    assert_eq!(counter["tags"], json!([]), "no tags");
    assert_eq!(
        counter["name"],
        json!({"key": null, "en": null, "de": null}),
        "no name"
    );
    Ok(())
}

#[test]
fn the_item_class_comes_from_the_class_line_of_the_english_description() -> TestResult {
    let (built, catalog) = catalog_with(&options())?;
    let class = |name: &str| -> Result<Value, String> {
        Ok(entity(&catalog, "items", name)?["itemClass"].clone())
    };
    assert_eq!(class("POWR_ACAS_S01")?, "Military", "a valid value");
    assert_eq!(
        class("BEHR_Rifle_Ballistic_01")?,
        Value::Null,
        "a damage type"
    );
    assert_eq!(class("Hat_01")?, Value::Null, "no Class: line");
    assert_eq!(class("Old_Box")?, Value::Null, "no English text");
    assert_eq!(
        built.summary().item_classes(),
        ItemClassSummary {
            classified: 1,
            other: 1,
            missing: 3,
        },
        "the summary counts both"
    );
    Ok(())
}

#[test]
fn the_item_classes_are_the_options_of_e_item_class() {
    assert_eq!(
        ItemClass::ALL.map(ItemClass::name),
        [
            "Civilian",
            "Competition",
            "Industrial",
            "Military",
            "Stealth"
        ],
        "EItemClass"
    );
    assert_eq!(
        ItemClass::from_name("stealth"),
        Some(ItemClass::Stealth),
        "ignoring case"
    );
    assert_eq!(ItemClass::from_name("Ballistic"), None, "another value");
}

#[test]
fn ships_are_entity_class_definitions_under_spaceships_and_groundvehicles_without_leftovers()
-> TestResult {
    let (built, catalog) = catalog_with(&options())?;
    assert_eq!(
        class_names(&catalog, "ships"),
        ["ACAS_Ship", "ACAS_Rover"],
        "the ship and the ground vehicle, by GUID"
    );
    let kind = built.summary().kind(Kind::Ships);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((2, 1, true)),
        "the template is skipped"
    );
    Ok(())
}

#[test]
fn a_ship_carries_names_role_career_crew_manufacturer_and_vehicle_definition() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    assert_eq!(
        catalog["ships"][0],
        json!({
            "guid": "90000000-0000-4000-8000-000000000001",
            "className": "ACAS_Ship",
            "recordPath": "libs/foundry/records/entities/spaceships/acas_ship",
            "name": {"key": "@vehicle_NameACAS_Ship", "en": "Ace Hauler", "de": null},
            "description": {"key": "@vehicle_DescACAS_Ship", "en": null, "de": null},
            "role": {"key": "@LOC_PLACEHOLDER", "en": null, "de": null},
            "career": {"key": "@vehicle_focus_transporter", "en": "Transporter", "de": null},
            "crewSize": 2,
            "manufacturerGuid": "10000000-0000-4000-8000-000000000001",
            "vehicleDefinition": "scripts/entities/vehicles/implementations/xml/acas_ship.xml",
            "playability": null,
            "variantOf": null,
            "vehicleKind": "spaceship",
            "size": null,
            "mass": null,
            "massBasis": null,
            "hullHealth": null,
            "cargoScu": 0.0,
            "cargoGridsWithIssue": 0,
            "insurance": null,
        }),
        "the ship fields"
    );
    Ok(())
}

#[test]
fn every_kind_states_its_completeness() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    assert_eq!(
        catalog["completeness"],
        json!({
            "manufacturers": {"status": "complete"},
            "items": {"status": "complete"},
            "ships": {"status": "complete"},
            "commodities": {"status": "complete"},
            "blueprints": {"status": "complete"},
            "resourceGroups": {"status": "complete"},
            "blueprintCategories": {"status": "complete"},
            "gameplayProperties": {"status": "complete"},
            "qualityDistributions": {"status": "complete"},
            "qualityQuantizations": {"status": "complete"},
            "blueprintPools": {"status": "complete"},
            "dismantleProcesses": {"status": "complete"},
            "paints": {"status": "complete"},
            "loadouts": {"status": "complete"},
            "starmapObjects": {"status": "complete"},
            "contracts": {"status": "complete"},
            "mineableElements": {"status": "complete"},
            "refiningProcesses": {"status": "complete"},
            "mineableCompositions": {"status": "complete"},
            "miningGlobalParams": {"status": "complete"},
            "harvestableProviderPresets": {"status": "complete"},
            "subHarvestableMultiConfigs": {"status": "complete"},
            "harvestableLocations": {"status": "complete"},
            "shops": {"status": "complete"},
            "factionReputations": {"status": "complete"},
            "reputationScopes": {"status": "complete"},
            "reputationStandings": {"status": "complete"},
            "reputationContexts": {"status": "complete"},
            "missionTypes": {"status": "complete"},
            "missionLocalities": {"status": "complete"},
            "contractScenarios": {"status": "complete"},
            "cargoManifests": {"status": "complete"},
            "missionLocationTemplates": {"status": "complete"},
            "contractShipSpawns": {"status": "complete"},
            "contractLocationQueries": {"status": "complete"},
            "contractTemplates": {"status": "complete"},
            "missionBrokerEntries": {"status": "complete"},
            "lootTables": {"status": "complete"},
            "lootArchetypes": {"status": "complete"},
            "lootPoolFilters": {"status": "complete"},
            "legacyLootTables": {"status": "complete"},
            "legacyLootArchetypes": {"status": "complete"},
            "lootContainers": {"status": "complete"},
            "mineableEntities": {"status": "complete"},
            "crafters": {"status": "complete"},
            "shipStoreSlugs": {"status": "complete"},
        }),
        "complete"
    );
    let filter = Filter::new("*/spaceships/*");
    let (built, catalog) = catalog_with(&CatalogOptions {
        filter: Some(&filter),
        ..options()
    })?;
    assert_eq!(
        catalog["completeness"]["items"],
        json!({"status": "partial", "filter": "*/spaceships/*"}),
        "partial"
    );
    assert_eq!(class_names(&catalog, "items"), Vec::<&str>::new(), "none");
    assert_eq!(class_names(&catalog, "ships"), ["ACAS_Ship"], "the ship");
    assert!(
        built.summary().kinds().iter().all(|kind| !kind.complete),
        "every kind partial"
    );
    Ok(())
}

#[test]
fn the_summary_lists_the_kinds_in_catalog_order() -> TestResult {
    let (built, _) = catalog_with(&options())?;
    let kinds: Vec<Kind> = built
        .summary()
        .kinds()
        .iter()
        .map(|kind| kind.kind)
        .collect();
    assert_eq!(kinds, Kind::ALL, "order");
    Ok(())
}
