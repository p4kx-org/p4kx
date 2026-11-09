// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Items and ships in the Basetool catalog of a real Archive; runs with `--ignored` when
//! `P4KX_DATA_P4K` names one.

use std::collections::HashSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogSummary, ItemClass, Kind, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::real_archive_path;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const MIN_ITEMS: usize = 10_000;
const MIN_SHIPS: usize = 200;
const MIN_RESOLVED_MANUFACTURERS: f64 = 0.95;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_item_classes_are_the_options_of_the_dataforge_enum() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let options = forge
        .enums()
        .find(|definition| definition.name().is_ok_and(|name| name == "EItemClass"))
        .ok_or("the enum EItemClass")?
        .options()
        .filter(|option| !matches!(option, Ok("UNDEFINED")))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        options,
        ItemClass::ALL.map(ItemClass::name),
        "the options of EItemClass besides UNDEFINED"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn builds_items_and_ships_of_a_real_archive() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let summary = built.summary();
    report(summary);
    for (kind, min) in [(Kind::Items, MIN_ITEMS), (Kind::Ships, MIN_SHIPS)] {
        let counts = summary.kind(kind).ok_or("the kind")?;
        assert!(counts.count >= min, "plausible {kind} count: {counts:?}");
        assert!(counts.complete, "{kind} complete without a Filter");
    }
    let manufacturers: HashSet<&str> = catalog["definitions"]["manufacturers"]["rows"]
        .as_array()
        .ok_or("manufacturer rows")?
        .iter()
        .map(|m| m["guid"]["value"].as_str().ok_or("manufacturer GUID"))
        .collect::<Result<_, _>>()?;
    for kind in ["items", "ships"] {
        let entities = list(&catalog, kind)?;
        let guids: Vec<&str> = entities.iter().filter_map(|e| e["guid"].as_str()).collect();
        assert!(guids.is_sorted(), "{kind} ordered by GUID");
        assert!(
            entities
                .iter()
                .filter_map(|e| e["recordPath"].as_str())
                .all(|path| !path.contains("_test_") && !path.contains("/test_")),
            "no test leftovers in {kind}"
        );
        let linked: Vec<&str> = entities
            .iter()
            .filter_map(|e| e["manufacturerGuid"].as_str())
            .collect();
        let resolved = linked
            .iter()
            .filter(|guid| manufacturers.contains(*guid))
            .count();
        assert!(
            ratio(resolved, linked.len()) >= MIN_RESOLVED_MANUFACTURERS,
            "{kind}: {resolved} of {} manufacturer GUIDs are catalog manufacturers",
            linked.len()
        );
    }
    check_items(&catalog, summary)?;
    check_ships(&catalog)
}

fn list<'c>(catalog: &'c Value, kind: &str) -> Result<&'c Vec<Value>, String> {
    catalog[kind]
        .as_array()
        .ok_or_else(|| format!("the {kind} array"))
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a share of counts far below 2^52"
)]
fn ratio(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        1.0
    } else {
        part as f64 / whole as f64
    }
}

fn check_items(catalog: &Value, summary: &CatalogSummary) -> TestResult {
    let items = list(catalog, "items")?;
    let classes: HashSet<&str> = items
        .iter()
        .filter_map(|item| item["itemClass"].as_str())
        .collect();
    for class in ItemClass::ALL {
        assert!(classes.contains(class.name()), "some item is {class:?}");
    }
    let counts = summary.item_classes();
    assert!(
        counts.classified > 0 && counts.other > 0 && counts.missing > 0,
        "classified, other and missing classes: {counts:?}"
    );
    let typed = items.iter().filter(|item| item["type"].is_string()).count();
    assert!(
        ratio(typed, items.len()) > 0.9,
        "most items have a type: {typed} of {}",
        items.len()
    );
    let massive = items.iter().filter(|item| item["mass"].is_number()).count();
    assert!(
        ratio(massive, items.len()) > 0.5,
        "most items have a mass: {massive} of {}",
        items.len()
    );
    Ok(())
}

fn check_ships(catalog: &Value) -> TestResult {
    let ships = list(catalog, "ships")?;
    let titan = ships
        .iter()
        .find(|ship| ship["className"] == "AEGS_Avenger_Titan")
        .ok_or("the Avenger Titan")?;
    assert!(
        titan["crewSize"].as_i64().is_some_and(|crew| crew >= 1),
        "crew"
    );
    assert!(
        titan["vehicleDefinition"]
            .as_str()
            .is_some_and(|path| path.to_ascii_lowercase().ends_with(".xml")),
        "vehicle definition"
    );
    assert!(titan["manufacturerGuid"].is_string(), "manufacturer");
    for text in ["name", "role", "career"] {
        assert!(
            titan
                .pointer(&format!("/{text}/key"))
                .and_then(Value::as_str)
                .is_some_and(|key| key.starts_with('@')),
            "{text} key"
        );
    }
    assert!(
        titan.pointer("/name/en").is_some_and(Value::is_string),
        "English name"
    );
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "the sizes and counts are the measurement the upload limit is sized from"
)]
fn report(summary: &CatalogSummary) {
    println!("catalog size: {} bytes", summary.size());
    for kind in summary.kinds() {
        println!("{kind:?}");
    }
    println!("{:?}", summary.item_classes());
    for text in summary.texts() {
        println!("{text:?}");
    }
}
