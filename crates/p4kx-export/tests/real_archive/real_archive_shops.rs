// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Shops in the P4KX catalog of a real Archive; runs with `--ignored` when `P4KX_DATA_P4K`
//! names one. The diagnostics print before any assertion.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Guid, Kind, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const MIN_SHOPS: usize = 1000;
const INVENTORY_FOLDER: &str = "data\\scripts\\shopinventories\\";
const MAX_INVENTORY_LEN: u64 = 64 << 20;
const LISTED: usize = 150;

fn count_by(shops: &[Value], text: impl Fn(&Value) -> Option<&str>) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for shop in shops {
        *counts.entry(text(shop).unwrap_or("(none)")).or_default() += 1;
    }
    counts
}

struct InventoryLink {
    name: String,
    chain_len: usize,
    items: usize,
    full: bool,
    last_matches: usize,
}

fn registry_text<'shop>(shop: &'shop Value, pointer: &str) -> Option<&'shop str> {
    shop.pointer(pointer).and_then(Value::as_str)
}
#[expect(
    clippy::print_stdout,
    reason = "the diagnostics are the result of the verification"
)]
fn inventories(
    archive: &Archive,
    shops: &[Value],
) -> Result<(usize, Vec<InventoryLink>), Box<dyn Error>> {
    let keys: HashSet<&str> = shops
        .iter()
        .filter_map(|shop| registry_text(shop, "/key/value"))
        .collect();
    let mut by_entity: HashMap<&str, usize> = HashMap::new();
    for shop in shops {
        *by_entity
            .entry(registry_text(shop, "/entityGuid/value").unwrap_or("(none)"))
            .or_default() += 1;
    }
    let mut without_shop_id = 0;
    let mut links = Vec::new();
    for entry in archive.entries() {
        let path = entry.path().replace('/', "\\").to_ascii_lowercase();
        let Some(name) = path.strip_prefix(INVENTORY_FOLDER) else {
            continue;
        };
        if !Path::new(name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let mut bytes = Vec::new();
        archive
            .open_entry(entry)?
            .take(MAX_INVENTORY_LEN)
            .read_to_end(&mut bytes)?;
        let Ok(json) = serde_json::from_slice::<Value>(&bytes) else {
            println!("   unreadable inventory {name}");
            continue;
        };
        let Some(chain) = json
            .pointer("/ShopID")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        else {
            without_shop_id += 1;
            continue;
        };
        let last = chain
            .rsplit(',')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        links.push(InventoryLink {
            name: name.to_owned(),
            chain_len: chain.split(',').count(),
            items: json
                .pointer("/Collection/Inventory")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            full: keys.contains(chain.to_ascii_lowercase().as_str()),
            last_matches: by_entity.get(last.as_str()).copied().unwrap_or(0),
        });
    }
    Ok((without_shop_id, links))
}

#[expect(
    clippy::print_stdout,
    reason = "the diagnostics are the result of the verification"
)]
fn print_inventories(archive: &Archive, shops: &[Value]) -> Result<(), Box<dyn Error>> {
    let (without, links) = inventories(archive, shops)?;
    let full = links.iter().filter(|link| link.full).count();
    let last_any = links.iter().filter(|link| link.last_matches > 0).count();
    let last_one = links.iter().filter(|link| link.last_matches == 1).count();
    println!(
        "== inventories: {} with a ShopID, {without} without; full chain equals a shop key: {full}; \
         last element equals a shop entity: {last_any} (exactly one shop: {last_one})",
        links.len()
    );
    for link in links.iter().take(LISTED) {
        println!(
            "   {} chain {} items {} full {} shops with its last element {}",
            link.name, link.chain_len, link.items, link.full, link.last_matches
        );
    }
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "the diagnostics are the result of the verification"
)]
fn print_default_values(archive: &Archive, shops: &[Value]) -> Result<(), Box<dyn Error>> {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let defaults: HashSet<&str> = shops
        .iter()
        .filter_map(|shop| registry_text(shop, "/defaultValue/value"))
        .collect();
    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    let mut resolved = 0;
    for default in &defaults {
        let Some(record) = default
            .parse::<Guid>()
            .ok()
            .and_then(|guid| forge.record_by_guid(guid))
        else {
            continue;
        };
        resolved += 1;
        let kind = record.type_name().unwrap_or("(unreadable)").to_owned();
        *by_type.entry(kind).or_default() += 1;
    }
    println!(
        "== defaultValue: {} distinct, {resolved} are Record GUIDs of the DataForge, by type {by_type:?}",
        defaults.len()
    );
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "the diagnostics are the result of the verification"
)]
fn report(archive: &Archive, built: &p4kx_export::BuiltCatalog, shops: &[Value]) -> TestResult {
    let summary = built.summary().shops();
    println!("== summary {summary:#?}");
    println!(
        "== shops per system {:?}",
        count_by(shops, |shop| registry_text(shop, "/system/value"))
    );
    println!(
        "== shops per kind {:?}",
        count_by(shops, |shop| shop
            .pointer("/kind/value")
            .and_then(Value::as_str))
    );
    println!(
        "== shops per brand {:?}",
        count_by(shops, |shop| shop
            .pointer("/brand/value")
            .and_then(Value::as_str))
    );
    let linked = shops
        .iter()
        .filter(|shop| {
            shop.pointer("/starmapObjectGuid/value")
                .is_some_and(|guid| !guid.is_null())
        })
        .count();
    println!(
        "== linked to a starmap object {linked}, unlinked {}",
        shops.len() - linked
    );
    let pyro = shops
        .iter()
        .filter(|shop| registry_text(shop, "/system/value") == Some("pyro"))
        .count();
    let pyro_linked = shops
        .iter()
        .filter(|shop| {
            registry_text(shop, "/system/value") == Some("pyro")
                && shop
                    .pointer("/starmapObjectGuid/value")
                    .is_some_and(|guid| !guid.is_null())
        })
        .count();
    println!("== Pyro shops {pyro}, linked {pyro_linked}");
    let exchanges: Vec<&str> = shops
        .iter()
        .filter(|shop| {
            shop.pointer("/commodityExchange/value")
                .and_then(Value::as_bool)
                == Some(true)
        })
        .map(|shop| registry_text(shop, "/entityName/value").unwrap_or("(none)"))
        .collect();
    println!("== commodity exchanges {}: {exchanges:?}", exchanges.len());
    println!(
        "== json bytes of shops: {}",
        Value::Array(shops.to_vec()).to_string().len()
    );
    print_inventories(archive, shops)?;
    print_default_values(archive, shops)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_shops_of_a_real_archive_have_a_kind_and_a_link() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(
        &archive,
        CatalogOptions::new(SystemTime::now()).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let shops = catalog["shops"].as_array().ok_or("shops")?;
    report(&archive, &built, shops)?;

    let summary = built.summary().shops();
    assert!(shops.len() >= MIN_SHOPS, "{} shops", shops.len());
    assert_eq!(summary.shops, shops.len(), "the summary counts shops");
    assert_eq!(
        built.summary().kind(Kind::Shops).map(|kind| kind.count),
        Some(shops.len()),
        "the kind summary"
    );
    assert!(!summary.registry_unreadable);
    assert_eq!(summary.containers_unreadable, 0, "{summary:?}");
    let keys: Vec<&str> = shops
        .iter()
        .filter_map(|shop| registry_text(shop, "/key/value"))
        .collect();
    assert!(keys.is_sorted(), "ordered by key");
    assert_eq!(
        keys.iter().collect::<HashSet<_>>().len(),
        keys.len(),
        "keys are unique"
    );
    assert!(summary.linked > 0, "{summary:?}");
    assert!(summary.with_kind > 0, "{summary:?}");
    assert!(summary.commodity_exchanges > 0, "{summary:?}");
    Ok(())
}
