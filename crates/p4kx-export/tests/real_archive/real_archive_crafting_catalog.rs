// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The crafting fields of the Basetool catalog of a real Archive: tiers, selections, quantity
//! multipliers, quality modifiers, pools with their reward sources, quality distributions and
//! dismantle returns; runs with `--ignored` when `P4KX_DATA_P4K` names one. No blueprint of a
//! real Archive has several tiers yet (checked on LIVE, PTU, EPTU, TECH-PREVIEW and HOTFIX), so
//! that case is covered by the synthetic `catalog_crafting_tiers` test; here a blueprint with
//! more than one tier would only have to stay consistent with the first-tier fields.

use std::collections::HashSet;
use std::error::Error;
use std::path::Path;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, UPLOAD_LIMIT, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_b_path, real_archive_path};
use serde_json::Value;

use crate::catalog_v3_consumer;

type TestResult = Result<(), Box<dyn Error>>;

const LMG_MAGAZINE: &str = "BP_CRAFT_behr_lmg_ballistic_01_mag";
const MULTIPLIER_BLUEPRINT: &str = "BP_CRAFT_slaver_armor_medium_arms_01_01_01";
const EXPECTED_MULTIPLIER: f64 = 1.15;
const CREATURE_DEFAULT: &str = "1dcde125-1275-474f-b0d7-d6682e9e521f";
const PYRO: &str = "286cb603-b4ae-4279-80a1-d4505fee1916";
const COVALEX_POOL: &str = "907989eb-9ee8-46a8-b794-032da6bd8a5d";
const COVALEX_BLUEPRINT: &str = "33be7f1a-3f75-4627-bb8b-88e6d6b42f4e";
const KINDS: [&str; 13] = [
    "manufacturers",
    "items",
    "ships",
    "commodities",
    "resourceTypes",
    "blueprints",
    "resourceGroups",
    "blueprintCategories",
    "gameplayProperties",
    "qualityDistributions",
    "qualityQuantizations",
    "blueprintPools",
    "dismantleProcesses",
];

fn list<'c>(catalog: &'c Value, kind: &str) -> Result<&'c Vec<Value>, String> {
    catalog_v3_consumer::named_rows(catalog, kind).map_err(|error| error.to_string())
}

fn named<'c>(catalog: &'c Value, class_name: &str) -> Result<&'c Value, String> {
    list(catalog, "blueprints")?
        .iter()
        .find(|blueprint| blueprint["className"] == class_name)
        .ok_or_else(|| format!("the blueprint {class_name}"))
}

fn at<'v>(value: &'v Value, pointer: &str) -> &'v Value {
    value.pointer(pointer).unwrap_or(&Value::Null)
}

fn lines(blueprint: &Value) -> Vec<&Value> {
    fn walk<'c>(line: &'c Value, out: &mut Vec<&'c Value>) {
        out.push(line);
        for option in line["options"].as_array().into_iter().flatten() {
            walk(option, out);
        }
    }
    let mut out = Vec::new();
    for tier in blueprint["tiers"].as_array().into_iter().flatten() {
        for line in std::iter::once(&tier["mandatory"])
            .chain(tier["optional"].as_array().into_iter().flatten())
        {
            if line.is_object() {
                walk(line, &mut out);
            }
        }
    }
    out
}

fn choose_fewer(line: &Value) -> bool {
    line["kind"] == "select"
        && line["count"].as_u64().is_some_and(|count| {
            usize::try_from(count)
                .is_ok_and(|count| count < line["options"].as_array().map_or(0, Vec::len))
        })
}

#[expect(
    clippy::print_stdout,
    reason = "the sizes are the measurement the upload limit is sized from"
)]
fn report(path: &Path, size: usize, catalog: &Value) -> TestResult {
    println!(
        "{}: catalog size {size} bytes, upload limit {UPLOAD_LIMIT}",
        path.display()
    );
    for kind in KINDS {
        let rows = list(catalog, kind)?;
        let bytes = serde_json::to_vec(rows)?.len();
        let count = rows.len();
        println!("  {kind}: {count} entries, {bytes} bytes");
    }
    for domain in ["publicNames", "gameTexts"] {
        let value = catalog
            .get("texts")
            .and_then(|texts| texts.get(domain))
            .ok_or("required canonical text table")?;
        let count = value.as_array().ok_or("canonical text table")?.len();
        let bytes = serde_json::to_vec(value)?.len();
        println!("  texts.{domain}: {count} entries, {bytes} shared bytes");
    }
    Ok(())
}

fn verify(path: &Path) -> TestResult {
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    report(path, built.json().len(), &catalog)?;
    assert!(
        crate::real_archive::is_complete_but_known_defects(built.summary()),
        "no failure, no warning but a known defect: {:?} {:?}",
        built.summary().failures().first(),
        built.summary().warnings()
    );
    check_tiers(&catalog)?;
    check_quality(&catalog)?;
    check_pools(&catalog)?;
    check_dismantle(&catalog)
}

fn flatten(line: &Value, slot: (&Value, &Value), out: &mut Vec<Value>) {
    match line["kind"].as_str() {
        Some("select") => {
            let own = (&line["slot"], &line["slotKey"]);
            let slot = if own.0.is_null() && own.1.is_null() {
                slot
            } else {
                own
            };
            for option in line["options"].as_array().into_iter().flatten() {
                flatten(option, slot, out);
            }
        }
        Some("resource" | "item") => out.push(serde_json::json!({
            "resourceGuid": line["resourceGuid"],
            "itemGuid": line["itemGuid"],
            "quantityScu": line["quantityScu"],
            "quantityUnits": line["quantityUnits"],
            "minQuality": line["minQuality"],
            "slot": slot.0,
            "slotKey": slot.1,
        })),
        _ => {}
    }
}

fn first_tier_ingredients(blueprint: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    let tier = at(blueprint, "/tiers/0");
    let costs = std::iter::once(&tier["mandatory"])
        .chain(tier["optional"].as_array().into_iter().flatten());
    for cost in costs.filter(|cost| cost.is_object()) {
        flatten(cost, (&Value::Null, &Value::Null), &mut out);
    }
    out
}

fn check_tiers(catalog: &Value) -> TestResult {
    let blueprints = list(catalog, "blueprints")?;
    let mut several = 0_usize;
    let mut fewer = 0_usize;
    for blueprint in blueprints {
        let tiers = blueprint["tiers"].as_array().ok_or("tiers")?;
        assert!(!tiers.is_empty(), "{} has a tier", blueprint["className"]);
        assert_eq!(
            at(blueprint, "/tiers/0/craftTimeSeconds"),
            &blueprint["craftTimeSeconds"],
            "{}: the first tier is the one craftTimeSeconds describes",
            blueprint["className"]
        );
        assert_eq!(
            blueprint["ingredients"].as_array(),
            Some(&first_tier_ingredients(blueprint)),
            "{}: the ingredients are the first tier flattened",
            blueprint["className"]
        );
        several += usize::from(tiers.len() > 1);
        fewer += usize::from(lines(blueprint).into_iter().any(choose_fewer));
    }
    assert!(fewer >= 1, "a blueprint that lets the player choose fewer");
    let magazine = named(catalog, LMG_MAGAZINE)?;
    let outer = at(magazine, "/tiers/0/mandatory");
    assert_eq!(
        (
            outer["kind"].as_str(),
            outer["count"].as_u64(),
            outer["options"].as_array().map(Vec::len)
        ),
        (Some("select"), Some(2), Some(2)),
        "choose 2 of 2"
    );
    assert_eq!(at(magazine, "/tiers/0/craftTimeSeconds"), 10, "craft time");
    let multiplied = lines(named(catalog, MULTIPLIER_BLUEPRINT)?)
        .into_iter()
        .filter_map(|line| line["quantityMultiplier"].as_f64())
        .any(|factor| (factor - EXPECTED_MULTIPLIER).abs() < 1.0e-6);
    assert!(multiplied, "the slaver arms carry the factor 1.15");
    print_line(&format!(
        "blueprints with several tiers: {several}, with a choice of fewer than offered: {fewer}"
    ));
    Ok(())
}

fn check_quality(catalog: &Value) -> TestResult {
    let properties: HashSet<&str> = list(catalog, "gameplayProperties")?
        .iter()
        .filter_map(|property| property["guid"].as_str())
        .collect();
    let mut kinds = HashSet::new();
    for blueprint in list(catalog, "blueprints")? {
        for line in lines(blueprint) {
            for modifier in line["modifiers"].as_array().into_iter().flatten() {
                if let Some(guid) = modifier["propertyGuid"].as_str() {
                    assert!(properties.contains(guid), "property {guid}");
                }
                for range in modifier["ranges"].as_array().into_iter().flatten() {
                    kinds.extend(range["kind"].as_str());
                }
            }
        }
    }
    assert!(
        kinds.contains("multiplier") && kinds.contains("additive"),
        "both range kinds occur: {kinds:?}"
    );
    let distributions = list(catalog, "qualityDistributions")?;
    let default = distributions
        .iter()
        .find(|item| item["guid"] == CREATURE_DEFAULT)
        .ok_or("the default creature distribution")?;
    assert_eq!(
        at(default, "/distribution/kind"),
        "CraftingQualityDistributionNormal",
        "kind"
    );
    assert_eq!(
        (
            at(default, "/distribution/min"),
            at(default, "/distribution/max"),
            at(default, "/distribution/mean"),
            at(default, "/distribution/stddev")
        ),
        (
            &Value::from(1.0),
            &Value::from(1000.0),
            &Value::from(269.0),
            &Value::from(254.0)
        ),
        "the curve"
    );
    assert!(
        distributions
            .iter()
            .flat_map(|item| item["overrides"].as_array().into_iter().flatten())
            .any(|over| over["locationGuid"] == PYRO && over["distribution"].is_object()),
        "an override for Pyro"
    );
    Ok(())
}

fn check_pools(catalog: &Value) -> TestResult {
    let pools = list(catalog, "blueprintPools")?;
    let pool = pools
        .iter()
        .find(|pool| pool["guid"] == COVALEX_POOL)
        .ok_or("the Covalex pool")?;
    let entries = pool["entries"].as_array().ok_or("entries")?;
    assert!(!entries.is_empty(), "entries");
    assert!(
        entries
            .iter()
            .any(|entry| entry["blueprintGuid"] == COVALEX_BLUEPRINT),
        "the Covalex blueprint is an entry"
    );
    let sources = pool["rewardSources"].as_array().ok_or("sources")?;
    assert!(!sources.is_empty(), "contracts reward the pool");
    assert!(
        sources
            .iter()
            .all(|source| source["recordPath"].is_string()),
        "every source names its Record"
    );
    let blueprints: HashSet<&str> = list(catalog, "blueprints")?
        .iter()
        .filter_map(|blueprint| blueprint["guid"].as_str())
        .collect();
    let (mut entries_total, mut unresolved) = (0_usize, 0_usize);
    for entry in pools
        .iter()
        .flat_map(|pool| pool["entries"].as_array().into_iter().flatten())
    {
        entries_total += 1;
        unresolved += usize::from(
            entry["blueprintGuid"]
                .as_str()
                .is_some_and(|guid| !blueprints.contains(guid)),
        );
    }
    let sources_total: usize = pools
        .iter()
        .map(|pool| pool["rewardSources"].as_array().map_or(0, Vec::len))
        .sum();
    print_line(&format!(
        "pools: {} with {entries_total} entries ({unresolved} to a blueprint outside the catalog) and {sources_total} reward sources",
        pools.len()
    ));
    Ok(())
}

fn check_dismantle(catalog: &Value) -> TestResult {
    let processes = list(catalog, "dismantleProcesses")?;
    let [process] = processes.as_slice() else {
        return Err(format!("one dismantle process, found {}", processes.len()).into());
    };
    assert_eq!(
        (&process["efficiency"], &process["timeSeconds"]),
        (&Value::from(0.5), &Value::from(15)),
        "efficiency and time"
    );
    let magazine = named(catalog, LMG_MAGAZINE)?;
    assert_eq!(
        at(magazine, "/dismantle/processGuid"),
        &process["guid"],
        "the process"
    );
    assert_eq!(at(magazine, "/dismantle/tierIndex"), 0, "the first tier");
    let excluded = at(catalog, "/craftingGlobal/dismantleExcludedResourceGuids")
        .as_array()
        .ok_or("excluded resources")?;
    assert!(!excluded.is_empty(), "excluded resources");
    let unlinked = list(catalog, "blueprints")?
        .iter()
        .filter(|blueprint| blueprint["dismantle"].is_null())
        .count();
    print_line(&format!(
        "blueprints without a dismantle return: {unlinked}"
    ));
    let defaults = list(catalog, "blueprints")?
        .iter()
        .filter(|blueprint| blueprint["defaultAvailable"] == true)
        .count();
    print_line(&format!("default blueprints in the catalog: {defaults}"));
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "the measurements are the result of the verification"
)]
fn print_line(line: &str) {
    println!("{line}");
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_crafting_fields_of_a_real_archive_are_plausible_and_consistent() -> TestResult {
    verify(&real_archive_path()?)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_crafting_fields_of_a_second_real_archive_are_plausible_and_consistent() -> TestResult {
    let Some(path) = real_archive_b_path() else {
        return Ok(());
    };
    verify(&path)
}
