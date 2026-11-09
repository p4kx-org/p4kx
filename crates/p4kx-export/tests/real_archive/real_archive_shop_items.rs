// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Item lists of shops in the catalog of a real Archive; runs with `--ignored` when
//! `P4KX_DATA_P4K` names one. The diagnostics print before any assertion.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Kind, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry, Guid};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::catalog_v3_consumer;

type TestResult = Result<(), Box<dyn Error>>;

const KINDS_WITH_ITEMS: [Kind; 5] = [
    Kind::Items,
    Kind::Ships,
    Kind::Commodities,
    Kind::Paints,
    Kind::Loadouts,
];
const PLACEHOLDERS: [&str; 2] = ["temp_outpost_shopinventory", "inv_cryastro_dummy"];

pub(crate) fn guids_of(catalog: &Value) -> Result<HashSet<&str>, Box<dyn Error>> {
    let mut result = HashSet::new();
    for kind in KINDS_WITH_ITEMS {
        for row in catalog_v3_consumer::rows(catalog, kind)? {
            result.insert(catalog_v3_consumer::identity(row, kind)?);
        }
    }
    Ok(result)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    clippy::too_many_lines,
    reason = "the diagnostics are the result of the verification, printed before the assertions"
)]
fn shops_get_item_lists_from_the_inventories_their_containers_name() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let shops = catalog["shops"].as_array().ok_or("shops")?;
    let summary = built.summary().shops();
    println!("== summary {summary:#?}");
    let with: Vec<&Value> = shops
        .iter()
        .filter(|shop| !shop["itemGuids"].is_null())
        .collect();
    let mut by_inventory: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for shop in &with {
        by_inventory
            .entry(shop["inventory"].as_str().unwrap_or("(none)"))
            .or_default()
            .push(shop["entityName"].as_str().unwrap_or_default());
    }
    println!(
        "== shops with an item list {} of {}; distinct inventories {} of {} files in the Archive ({} named by containers or prefabs, placeholders included)",
        with.len(),
        shops.len(),
        by_inventory.len(),
        summary.inventories_in_archive,
        summary.inventories_named
    );
    for (name, entities) in &by_inventory {
        println!("   {name}: {} shops", entities.len());
    }
    let mut pairs: Vec<(&str, &str, &str)> = with
        .iter()
        .map(|shop| {
            (
                shop["entityName"].as_str().unwrap_or_default(),
                shop["location"].as_str().unwrap_or_default(),
                shop["inventory"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    pairs.sort_unstable();
    pairs.dedup();
    println!("== shop (location) -> file, {} distinct pairs", pairs.len());
    for (name, location, file) in &pairs {
        println!("   {name} ({location}) -> {file}");
    }
    println!(
        "== soc paths without an Entity {}; prefab Entities {} of which ambiguous {}; shops listed by a prefab alone {}",
        summary.soc_paths_unattributed,
        summary.prefab_entities,
        summary.prefab_ambiguous,
        summary.prefab_only_shops
    );
    println!(
        "== not exported: {} shops name a placeholder, {} are ambiguous, {} unscanned, {} name a file that is missing, {} an unreadable one",
        summary.inventory_placeholder,
        summary.inventory_ambiguous,
        summary.inventory_unscanned,
        summary.inventory_missing,
        summary.inventory_unreadable
    );
    let known = guids_of(&catalog)?;
    let mut total = 0_usize;
    let mut resolved = 0_usize;
    let mut unresolved: BTreeSet<&str> = BTreeSet::new();
    let mut empty = 0_usize;
    let mut inventories_seen: HashSet<&str> = HashSet::new();
    for shop in &with {
        let items = shop["itemGuids"].as_array().ok_or("itemGuids")?;
        empty += usize::from(items.is_empty());
        if !inventories_seen.insert(shop["inventory"].as_str().unwrap_or_default()) {
            continue;
        }
        for item in items.iter().filter_map(Value::as_str) {
            total += 1;
            if known.contains(item) {
                resolved += 1;
            } else {
                unresolved.insert(item);
            }
        }
    }
    println!(
        "== item GUIDs over distinct inventories: {total}, found in items/ships/commodities/paints/loadouts: {resolved}, distinct not found: {}, empty lists: {empty}",
        unresolved.len()
    );
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut records = 0_usize;
    for item in &unresolved {
        match item
            .parse::<Guid>()
            .ok()
            .and_then(|guid| forge.record_by_guid(guid))
        {
            Some(record) => {
                records += 1;
                println!(
                    "   not in the catalog {item}: Record {} {}",
                    record.type_name().unwrap_or("?"),
                    record.name().unwrap_or("?")
                );
            }
            None => println!("   not in the catalog {item}: no Record has the GUID"),
        }
    }
    println!(
        "== of the {} GUIDs not in the catalog {records} are a Record of another type",
        unresolved.len()
    );
    let by_system: BTreeMap<&str, usize> = with.iter().fold(BTreeMap::new(), |mut counts, shop| {
        *counts
            .entry(shop["system"].as_str().unwrap_or("(none)"))
            .or_default() += 1;
        counts
    });
    println!("== shops with an item list per system {by_system:?}");
    let text = serde_json::to_string(&catalog["shops"])?;
    for field in [
        "BuyPrice",
        "SellPrice",
        "CurrentInventory",
        "RentalOfferings",
    ] {
        assert!(!text.contains(field), "{field} leaked into the shops");
    }
    for name in by_inventory.keys() {
        assert!(!PLACEHOLDERS.contains(name), "{name} is a placeholder");
    }
    assert!(summary.with_items > 0, "{summary:?}");
    assert_eq!(summary.with_items, with.len());
    assert!(summary.with_items >= 250, "{summary:?}");
    assert!(summary.inventory_placeholder > 0, "{summary:?}");
    assert!(summary.inventory_unscanned < 144, "{summary:?}");
    assert_eq!(summary.other_containers_unreadable, 0, "{summary:?}");
    assert_eq!(summary.prefabs_unreadable, 0, "{summary:?}");
    assert_eq!(summary.inventories_missing, 0, "{summary:?}");
    assert!(summary.soc_components > 0, "{summary:?}");
    assert!(resolved * 10 >= total * 9, "{resolved} of {total} resolve");
    assert_eq!(records, 0, "an unknown GUID is a Record");
    Ok(())
}
