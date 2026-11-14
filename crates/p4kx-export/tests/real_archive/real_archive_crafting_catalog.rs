// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The crafting fields of the Basetool catalog of a real Archive: tiers, selections, quantity
//! multipliers, quality modifiers, pools with their reward sources, quality distributions and
//! dismantle returns; runs with `--ignored` when `P4KX_DATA_P4K` names one. No blueprint of a
//! real Archive has several tiers yet (checked on LIVE, PTU, EPTU, TECH-PREVIEW and HOTFIX), so
//! that case is covered by the synthetic `catalog_crafting_tiers` test; here a blueprint with
//! more than one tier would only have to stay consistent with the first-tier fields.

#[path = "real_archive_crafting_quality.rs"]
mod quality;

use std::collections::HashSet;
use std::error::Error;
use std::path::Path;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Kind, UPLOAD_LIMIT, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_b_path, real_archive_path};
use serde_json::Value;

use crate::catalog_v3_consumer;
use crate::real_archive_blueprints::Blueprints;

type TestResult = Result<(), Box<dyn Error>>;

const LMG_MAGAZINE: &str = "BP_CRAFT_behr_lmg_ballistic_01_mag";
const MULTIPLIER_BLUEPRINT: &str = "BP_CRAFT_slaver_armor_medium_arms_01_01_01";
const EXPECTED_MULTIPLIER: f64 = 1.15;
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

fn at<'v>(value: &'v Value, pointer: &str) -> &'v Value {
    value.pointer(pointer).unwrap_or(&Value::Null)
}

fn choose_fewer(line: &Value) -> bool {
    let meaning = &line["meaning"];
    meaning["type"] == "select"
        && meaning
            .pointer("/count/value")
            .and_then(Value::as_str)
            .and_then(|count| count.parse::<usize>().ok())
            .is_some_and(|count| count < meaning["options"].as_array().map_or(0, Vec::len))
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
    let blueprints = Blueprints::read(&catalog)?;
    check_tiers(&blueprints)?;
    check_quality(&catalog, &blueprints)?;
    check_pools(&catalog)?;
    check_dismantle(&catalog, &blueprints)
}

fn check_tiers(blueprints: &Blueprints<'_>) -> TestResult {
    let mut several = 0_usize;
    let mut fewer = 0_usize;
    for blueprint in blueprints.rows() {
        assert!(
            !blueprint.tiers.is_empty(),
            "{} has a tier",
            blueprint.definition["className"]
        );
        blueprint.check_ingredients()?;
        several += usize::from(blueprint.tiers.len() > 1);
        fewer += usize::from(blueprint.costs().any(choose_fewer));
    }
    assert!(fewer >= 1, "a Blueprint lets the player choose fewer");
    let magazine = blueprints.named(LMG_MAGAZINE)?;
    let outer = &magazine.first_cost()?["meaning"];
    assert_eq!(outer["type"], "select", "the magazine has a selection cost");
    assert_eq!(
        outer.pointer("/count/value").and_then(Value::as_str),
        Some("2"),
        "the magazine selection requires two options"
    );
    assert_eq!(
        outer["options"].as_array().map(Vec::len),
        Some(2),
        "both magazine options are present"
    );
    assert_eq!(
        magazine
            .tiers
            .first()
            .and_then(|tier| tier.pointer("/craftTimeSeconds/value"))
            .and_then(Value::as_str),
        Some("10"),
        "the magazine takes ten seconds"
    );
    let multiplied = blueprints
        .named(MULTIPLIER_BLUEPRINT)?
        .costs()
        .filter_map(|line| {
            line.pointer("/context/quantityMultiplier/value")
                .and_then(Value::as_f64)
        })
        .any(|factor| (factor - EXPECTED_MULTIPLIER).abs() < 1.0e-6);
    assert!(multiplied, "the slaver arms carry the factor 1.15");
    print_line(&format!(
        "blueprints with several tiers: {several}, with a choice of fewer than offered: {fewer}"
    ));
    Ok(())
}

fn check_quality(catalog: &Value, blueprints: &Blueprints<'_>) -> TestResult {
    let properties: HashSet<&str> = list(catalog, "gameplayProperties")?
        .iter()
        .map(|property| catalog_v3_consumer::identity(property, Kind::GameplayProperties))
        .collect::<Result<_, _>>()?;
    let mut kinds = HashSet::new();
    for blueprint in blueprints.rows() {
        for line in blueprint.costs() {
            for modifier in line
                .pointer("/context/modifiers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(guid) = modifier
                    .pointer("/propertyGuid/value")
                    .and_then(Value::as_str)
                {
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
    quality::check_sources(catalog)
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
        .filter_map(|blueprint| blueprint.pointer("/guid/value").and_then(Value::as_str))
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

fn check_dismantle(catalog: &Value, blueprints: &Blueprints<'_>) -> TestResult {
    let processes = list(catalog, "dismantleProcesses")?;
    let [process] = processes.as_slice() else {
        return Err(format!("one dismantle process, found {}", processes.len()).into());
    };
    assert_eq!(
        (&process["efficiency"], &process["timeSeconds"]),
        (&Value::from(0.5), &Value::from(15)),
        "efficiency and time"
    );
    let magazine = blueprints.named(LMG_MAGAZINE)?;
    assert_eq!(
        magazine
            .presentation
            .pointer("/dismantle/value/processGuid")
            .ok_or("magazine dismantle process GUID")?,
        &process["guid"],
        "the process"
    );
    assert_eq!(
        magazine.presentation.pointer("/dismantle/value/tier"),
        magazine.presentation.get("firstTier"),
        "the dismantle return uses the first tier"
    );
    let excluded = at(catalog, "/craftingGlobal/dismantleExcludedResourceGuids")
        .as_array()
        .ok_or("excluded resources")?;
    assert!(!excluded.is_empty(), "excluded resources");
    let unlinked = blueprints
        .rows()
        .iter()
        .filter(|blueprint| {
            blueprint
                .presentation
                .pointer("/dismantle/value")
                .is_some_and(Value::is_null)
        })
        .count();
    print_line(&format!(
        "blueprints without a dismantle return: {unlinked}"
    ));
    let defaults = blueprints
        .rows()
        .iter()
        .filter(|blueprint| {
            blueprint
                .presentation
                .pointer("/defaultAvailable/value")
                .and_then(Value::as_bool)
                == Some(true)
        })
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
