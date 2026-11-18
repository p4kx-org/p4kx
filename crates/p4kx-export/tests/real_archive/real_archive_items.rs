// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Items and ships in the P4KX catalog of a real Archive; runs with `--ignored` when
//! `P4KX_DATA_P4K` names one.

use std::collections::HashSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogSummary, ItemClass, Kind, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use crate::catalog_v3_consumer;
use crate::real_archive_canonical_texts::ItemPresentationIndex;

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
    check_entity_links(&catalog)?;
    check_items(&catalog, summary)?;
    check_ships(&catalog)
}

fn check_entity_links(catalog: &Value) -> TestResult {
    let manufacturers: HashSet<&str> = catalog_v3_consumer::rows(catalog, Kind::Manufacturers)?
        .iter()
        .map(|row| catalog_v3_consumer::identity(row, Kind::Manufacturers))
        .collect::<Result<_, _>>()?;
    let originals = ItemPresentationIndex::new(catalog)?;
    for kind in [Kind::Items, Kind::Ships] {
        check_entity_family(catalog, kind, &manufacturers, &originals)?;
    }
    Ok(())
}

fn check_entity_family(
    catalog: &Value,
    kind: Kind,
    manufacturers: &HashSet<&str>,
    originals: &ItemPresentationIndex<'_>,
) -> TestResult {
    let entities = list(catalog, kind.name())?;
    let guids: Vec<_> = entities
        .iter()
        .map(|entity| catalog_v3_consumer::identity(entity, kind))
        .collect::<Result<_, _>>()?;
    assert!(guids.is_sorted(), "{kind} ordered by GUID");
    let mut linked = Vec::new();
    for entity in entities {
        let path = catalog_v3_consumer::field(entity, kind, "recordPath")?
            .as_str()
            .ok_or("original entity Record path")?;
        assert!(
            !path.contains("_test_") && !path.contains("/test_"),
            "no test leftovers in {kind}"
        );
        let manufacturer = if kind == Kind::Ships {
            crate::real_archive_catalog_links::vehicle_manufacturer(entity)
        } else {
            originals.manufacturer(entity)?
        };
        if let Some(guid) = manufacturer {
            linked.push(guid);
        }
    }
    let resolved = linked
        .iter()
        .filter(|guid| manufacturers.contains(*guid))
        .count();
    assert!(
        ratio(resolved, linked.len()) >= MIN_RESOLVED_MANUFACTURERS,
        "{kind}: {resolved} of {} manufacturer GUIDs are catalog manufacturers",
        linked.len()
    );
    Ok(())
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
    let originals = ItemPresentationIndex::new(catalog)?;
    let mut classes = HashSet::new();
    let mut typed = 0_usize;
    let mut massive = 0_usize;
    for item in items {
        let class = item.get("itemClass").ok_or("sourced Item class")?;
        if let Some(class) = catalog_v3_consumer::source_value(class)? {
            classes.insert(class.as_str().ok_or("derived Item class label")?);
        }
        typed += usize::from(
            originals
                .item_type(item)?
                .is_some_and(|label| !label.is_empty()),
        );
        massive += usize::from(originals.mass(item)?.is_some());
    }
    for class in ItemClass::ALL {
        assert!(classes.contains(class.name()), "some item is {class:?}");
    }
    let counts = summary.item_classes();
    assert!(
        counts.classified > 0 && counts.other > 0 && counts.missing > 0,
        "classified, other and missing classes: {counts:?}"
    );
    assert!(
        ratio(typed, items.len()) > 0.9,
        "most items have a type: {typed} of {}",
        items.len()
    );
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
    assert!(
        crate::real_archive_catalog_links::vehicle_manufacturer(titan).is_some(),
        "authored vehicle manufacturer"
    );
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
