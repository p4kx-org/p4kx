// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Items and ships in the P4KX catalog: selection, fields and item class, through the public
//! API.

use crate::catalog_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use catalog_fixture::{Maker, english, write_archive, write_archive_with};
use p4kx_export::{BuiltCatalog, CatalogOptions, ItemClass, ItemClassSummary, Kind, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[path = "catalog_items_completeness.rs"]
mod completeness;
#[path = "catalog_item_definitions.rs"]
mod definitions;
#[path = "catalog_ship_texts.rs"]
mod ship_texts;

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
    catalog[kind].as_array().map_or_default(|list| {
        list.iter()
            .filter(|entry| entry["className"] != "Counter_Shop")
            .map(|entry| &entry["manufacturerGuid"])
            .collect()
    })
}

#[test]
fn a_manufacturer_given_by_reference_is_its_guid() -> TestResult {
    let catalog = catalog_of(Maker::Reference)?;
    assert!(catalog.get("manufacturers").is_none());
    let maker = &catalog["definitions"]["manufacturers"]["rows"][0]["guid"]["value"];
    assert_eq!(maker, "10000000-0000-4000-8000-000000000001");
    for kind in ["items", "ships"] {
        let guids = manufacturer_guids(&catalog, kind);
        assert!(!guids.is_empty(), "{kind} present");
        assert!(guids.iter().all(|guid| *guid == maker), "{kind}: {guids:?}");
    }
    Ok(())
}

#[test]
fn a_manufacturer_given_by_a_strong_or_weak_pointer_is_null() -> TestResult {
    for maker in [Maker::StrongPointer, Maker::WeakPointer] {
        let catalog = catalog_of(maker)?;
        let rows = catalog["definitions"]["items"]["rows"]
            .as_array()
            .ok_or("canonical items")?;
        let item = rows
            .iter()
            .find(|row| row.pointer("/className/value") == Some(&json!("POWR_ACAS_S01")))
            .ok_or("canonical item")?;
        let attachment = item
            .pointer("/components/value")
            .and_then(Value::as_array)
            .and_then(|rows| rows.iter().find(|row| row["kind"] == "attachment"))
            .ok_or("source attachment")?;
        let manufacturer = attachment
            .pointer("/value/value/manufacturer")
            .ok_or("authored manufacturer pointer")?;
        let raw = manufacturer
            .pointer("/value/value/values/0")
            .ok_or("original pointer fact")?;
        assert_eq!(
            manufacturer["kind"], "authored",
            "retain the original pointer shape"
        );
        match maker {
            Maker::StrongPointer => assert_eq!(
                raw.pointer("/value/value/type"),
                Some(&json!("struct")),
                "retain an owned manufacturer value without inventing a Record reference",
            ),
            Maker::WeakPointer => assert_eq!(
                raw.pointer("/originalError/type"),
                Some(&json!("outsideRecord")),
                "retain the original outside-owner fact without blocking publication",
            ),
            Maker::Reference => return Err("a pointer fixture is required".into()),
        }
        for kind in ["items", "ships"] {
            let guids = manufacturer_guids(&catalog, kind);
            assert!(!guids.is_empty(), "{kind} present");
            assert!(guids.iter().all(|guid| guid.is_null()), "{kind}: {guids:?}");
        }
    }
    Ok(())
}

fn class_names<'c>(catalog: &'c Value, kind: &str) -> Vec<&'c str> {
    catalog[kind].as_array().map_or_default(|list| {
        list.iter()
            .filter_map(|entry| entry["className"].as_str())
            .collect()
    })
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
    let projection = entity(&catalog, "items", "POWR_ACAS_S01")?;
    let definition = item_definition(&catalog, projection)?;
    let mut actual = projection.clone();
    if let Some(fields) = actual.as_object_mut() {
        fields.remove("variantOf");
        fields.remove("recordIndex");
    }
    assert_eq!(
        &actual,
        &json!({
            "guid": "30000000-0000-4000-8000-000000000001",
            "className": "POWR_ACAS_S01",
            "recordPath": "libs/foundry/records/entities/scitem/ships/powerplant/powr_acas_s01",
            "type": "PowerPlant",
            "subType": "Power",
            "size": 1,
            "grade": 3,
            "mass": 125.5,
            "manufacturerGuid": "10000000-0000-4000-8000-000000000001",
            "tags": ["flightReady", "$Small"],
            "itemClass": "Military",
            "playability": null,
        }),
        "the remaining item projection fields"
    );
    check_item_texts(&catalog, definition)
}

fn item_definition<'a>(
    catalog: &'a Value,
    projection: &Value,
) -> Result<&'a Value, Box<dyn Error>> {
    let index = projection
        .get("recordIndex")
        .ok_or("sourced physical item index")?;
    crate::health_origin_assertions::rows(catalog, "/definitions/items/rows")?
        .iter()
        .find(|definition| definition.get("recordIndex") == Some(index))
        .ok_or_else(|| "exact Item definition".into())
}

fn check_item_texts(catalog: &Value, definition: &Value) -> TestResult {
    let components = crate::health_origin_assertions::rows(definition, "/components/value")?;
    let attachment = components
        .iter()
        .find(|component| component["kind"] == "attachment")
        .ok_or("first attachment")?;
    let purchase = components
        .iter()
        .find(|component| component["kind"] == "purchasable")
        .ok_or("first purchasable component")?;
    for (carrier, table, key, english) in [
        (
            attachment.pointer("/value/value/localization/value/name/value"),
            "publicNames",
            "@item_NamePOWR_ACAS_S01",
            "Lumen",
        ),
        (
            attachment.pointer("/value/value/localization/value/description/value"),
            "gameTexts",
            "@item_DescPOWR_ACAS_S01",
            "Item Type: Power Plant\nClass:\u{a0}Military \n\nSteady power.",
        ),
        (
            purchase.pointer("/value/displayType/value"),
            "gameTexts",
            "@item_DisplayType_PowerPlant",
            "Power Plant",
        ),
    ] {
        let id = carrier.ok_or("selected Item text reference")?;
        let texts = crate::health_origin_assertions::rows(catalog, &format!("/texts/{table}"))?;
        let text = texts
            .iter()
            .find(|row| row.pointer("/id/value") == Some(id))
            .ok_or("canonical Item text target")?;
        assert_eq!(
            text.pointer("/key/value"),
            Some(&json!(key)),
            "original Item text key"
        );
        assert_eq!(
            text.pointer("/translations/value/en/displayText/value"),
            Some(&json!(english)),
            "normalized English Item display text"
        );
        assert!(
            text.pointer("/translations/value/de").is_none(),
            "German was not loaded"
        );
    }
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
    let definition = item_definition(&catalog, counter)?;
    let mut components = definition
        .pointer("/components/value")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    assert!(
        components.all(|entry| entry["kind"] != "attachment"),
        "the Item has no attachment name carrier"
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
fn a_ship_carries_crew_manufacturer_and_vehicle_definition() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    let mut ship = catalog["ships"][0].as_object().ok_or("ship")?.clone();
    for text in ["name", "description", "role", "career"] {
        ship.remove(text);
    }
    assert_eq!(
        Value::Object(ship),
        json!({
            "guid": "90000000-0000-4000-8000-000000000001",
            "className": "ACAS_Ship",
            "recordPath": "libs/foundry/records/entities/spaceships/acas_ship",
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
