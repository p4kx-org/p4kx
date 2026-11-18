// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Items and ships in the P4KX catalog: selection, fields and item class, through the public
//! API.

use crate::catalog_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use catalog_fixture::{english, write_archive};
use p4kx_export::{BuiltCatalog, CatalogOptions, ItemClass, ItemClassSummary, Kind, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[path = "catalog_items_completeness.rs"]
mod completeness;
#[path = "catalog_item_definitions.rs"]
pub(crate) mod definitions;
#[path = "catalog_item_projection_checks.rs"]
mod item_projection_checks;
#[path = "catalog_ship_manufacturers.rs"]
mod ship_manufacturers;
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

fn class_names<'c>(catalog: &'c Value, kind: &str) -> Vec<&'c str> {
    catalog[kind].as_array().map_or_default(|list| {
        list.iter()
            .filter_map(|entry| entity_field(entry, kind, "className").and_then(Value::as_str))
            .collect()
    })
}

fn entity<'c>(catalog: &'c Value, kind: &str, class: &str) -> Result<&'c Value, String> {
    catalog[kind]
        .as_array()
        .and_then(|list| {
            list.iter().find(|entry| {
                entity_field(entry, kind, "className").and_then(Value::as_str) == Some(class)
            })
        })
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
    item_projection_checks::original_values(&catalog, projection, definition)?;
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
        "tags",
    ] {
        assert!(
            counter.get(field).is_none(),
            "{field} has one original source-family location"
        );
    }
    assert_eq!(
        counter.pointer("/itemClass/state"),
        Some(&json!("absent")),
        "no class without a source definition"
    );
    assert_eq!(
        counter.pointer("/itemClass/value"),
        Some(&Value::Null),
        "missing class is explicitly nullable"
    );
    assert_eq!(
        counter.pointer("/selection/attachmentComponentOrdinal/state"),
        Some(&json!("absent")),
        "complete original absence is retained"
    );
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
        Ok(entity(&catalog, "items", name)?["itemClass"]["value"].clone())
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
fn a_ship_carries_crew_and_vehicle_definition() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    let ship = &catalog["ships"][0];
    let proof = crate::health_origin_assertions::present;
    proof(
        ship,
        "guid",
        &json!("90000000-0000-4000-8000-000000000001"),
        Some(true),
    )?;
    proof(ship, "className", &json!("ACAS_Ship"), Some(true))?;
    proof(
        ship,
        "recordPath",
        &json!("libs/foundry/records/entities/spaceships/acas_ship"),
        Some(true),
    )?;
    proof(
        ship,
        "crewSize",
        &json!({"type": "int32", "value": 2}),
        Some(true),
    )?;
    proof(
        ship,
        "vehicleDefinition",
        &json!("scripts/entities/vehicles/implementations/xml/acas_ship.xml"),
        Some(true),
    )?;
    for field in ["size", "insurance"] {
        crate::health_origin_assertions::cell(ship, field, "absent")?;
    }
    for field in ["playability", "variantOf"] {
        assert!(ship[field].is_null(), "the fixture has no {field}");
    }
    for field in ["mass", "massBasis", "hullHealth"] {
        crate::health_origin_assertions::cell(ship, field, "unreadable")?;
    }
    assert_eq!(ship["vehicleKind"], "spaceship");
    assert_eq!(ship["cargoScu"], 0.0);
    assert_eq!(ship["cargoGridsWithIssue"], 0);
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

pub(crate) fn entity_field<'a>(row: &'a Value, kind: &str, field: &str) -> Option<&'a Value> {
    match kind {
        "items" => row.get(field)?.get("value"),
        "ships"
            if matches!(
                field,
                "guid"
                    | "recordName"
                    | "className"
                    | "recordPath"
                    | "recordIndex"
                    | "sourceType"
                    | "fileName"
                    | "metadataTag"
            ) =>
        {
            row.get(field)?.get("value")
        }
        "ships" | "crafters" => row.get(field),
        _ => None,
    }
}

pub(crate) fn selected_attachment<'a>(
    catalog: &'a Value,
    projection: &Value,
) -> Result<&'a Value, Box<dyn Error>> {
    let definition = item_definition(catalog, projection)?;
    let slot = projection
        .pointer("/selection/attachmentComponentOrdinal/value")
        .ok_or("selected attachment slot")?;
    definition
        .pointer("/components/value")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.pointer("/componentOrdinal/value") == Some(slot))
        })
        .and_then(|row| row.pointer("/value/value"))
        .ok_or_else(|| "exact physically selected source attachment".into())
}
