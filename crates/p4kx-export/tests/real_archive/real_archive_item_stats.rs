// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The typed stat blocks of items in the Basetool catalog of a real Archive; runs with
//! `--ignored` when `P4KX_DATA_P4K` names one.

use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::real_archive_path;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const BLOCKS: [(&str, &[&str]); 6] = [
    ("weapon", &["fireActions", "ammoCapacity", "ammo"]),
    (
        "quantumDrive",
        &["fuelRequirement", "jumpRange", "driveSpeed", "spoolUpTime"],
    ),
    ("powerPlant", &["powerOutput", "unit"]),
    ("cooler", &["coolingRate", "unit"]),
    (
        "missile",
        &["damage", "explosionMaxRadius", "lockTime", "linearSpeed"],
    ),
    ("resourceStates", &[]),
];

const MIN_SHARE: f64 = 0.8;

const fn present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Array(items) => !items.is_empty(),
        _ => true,
    }
}

#[expect(clippy::print_stdout, reason = "the numbers are the result of the run")]
fn report(line: &str) {
    println!("{line}");
}

fn catalog() -> Result<Value, Box<dyn Error>> {
    let archive = Archive::open_with_key(real_archive_path()?, None)?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    report(&format!("catalog size: {} bytes", built.json().len()));
    Ok(serde_json::from_slice(built.json())?)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_stat_block_is_filled_for_the_items_that_have_it() -> TestResult {
    let catalog = catalog()?;
    let items = catalog["items"].as_array().ok_or("the items array")?;
    let with_stats = items.iter().filter(|item| !item["stats"].is_null()).count();
    report(&format!("{with_stats} of {} items have stats", items.len()));
    let mut problems = Vec::new();
    for (block, fields) in BLOCKS {
        let holders: Vec<&Value> = items
            .iter()
            .filter(|item| present(&item["stats"][block]))
            .collect();
        report(&format!("{block}: {} items", holders.len()));
        if holders.is_empty() {
            problems.push(format!("no item has a {block} block"));
            continue;
        }
        for field in fields {
            let filled = holders
                .iter()
                .filter(|item| present(&item["stats"][block][field]))
                .count();
            #[expect(clippy::cast_precision_loss, reason = "counts far below 2^52")]
            let share = filled as f64 / holders.len() as f64;
            report(&format!("  {field}: {filled} of {} filled", holders.len()));
            if share < MIN_SHARE {
                problems.push(format!("{block}.{field}: {filled} of {}", holders.len()));
            }
        }
        if let Some(sample) = holders.iter().find(|item| {
            fields
                .iter()
                .all(|field| present(&item["stats"][block][field]))
        }) {
            report(&format!(
                "  sample {}: {}",
                sample["className"],
                serde_json::to_string(&sample["stats"])?
            ));
        }
    }
    check_shield_coverage(&catalog, &mut problems)?;
    assert!(problems.is_empty(), "blocks below 80 %: {problems:?}");
    Ok(())
}

fn shields(catalog: &Value) -> Result<&[Value], Box<dyn Error>> {
    catalog
        .pointer("/configurations/shieldGenerators/rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "canonical shield generator rows".into())
}

fn check_shield_coverage(catalog: &Value, problems: &mut Vec<String>) -> TestResult {
    let rows = shields(catalog)?;
    report(&format!("shield generators: {} rows", rows.len()));
    if rows.is_empty() {
        problems.push("no canonical shield generator row".to_owned());
        return Ok(());
    }
    for field in [
        "maxHealth",
        "maxRegen",
        "downedRegenDelay",
        "damagedRegenDelay",
    ] {
        let pointer = format!("/{field}/value/finite");
        let filled = rows
            .iter()
            .filter(|row| row.pointer(&pointer).is_some_and(Value::is_number))
            .count();
        #[expect(clippy::cast_precision_loss, reason = "counts far below 2^52")]
        let share = filled as f64 / rows.len() as f64;
        report(&format!("  {field}: {filled} of {} filled", rows.len()));
        if share < MIN_SHARE {
            problems.push(format!(
                "shieldGenerators.{field}: {filled} of {}",
                rows.len()
            ));
        }
    }
    Ok(())
}

fn item_of<'c>(items: &'c [Value], class: &str) -> Result<&'c Value, String> {
    items
        .iter()
        .find(|item| item["className"] == class)
        .ok_or_else(|| format!("no item {class}"))
}

fn stats_of<'c>(items: &'c [Value], class: &str) -> Result<&'c Value, String> {
    Ok(&item_of(items, class)?["stats"])
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn known_items_carry_their_known_values() -> TestResult {
    let catalog = catalog()?;
    let items = catalog["items"].as_array().ok_or("the items array")?;
    let item = item_of(items, "SHLD_GODI_S01_FR66_SCItem")?;
    let guid = item.get("guid").ok_or("known shield item GUID")?;
    let shield = shields(&catalog)?
        .iter()
        .find(|row| row.pointer("/ownerGuid/value") == Some(guid))
        .ok_or("known shield generator configuration")?;
    assert_eq!(
        shield
            .pointer("/maxHealth/value/finite")
            .and_then(Value::as_f64),
        Some(3_168.0),
        "known original shield health in the canonical configuration"
    );
    let missile = stats_of(items, "MISL_S07_IR_TALN_Hellion")?;
    assert_eq!(missile["missile"]["trackingSignalType"], "Infrared");
    let cooler = stats_of(items, "COOL_JUST_S02_Snowpack_SCItem")?;
    assert_eq!(cooler["cooler"]["coolingRate"], 46.0);
    let plant = stats_of(items, "POWR_JUST_S03_Durango_SCItem")?;
    assert_eq!(plant["powerPlant"]["powerOutput"], 24.0);
    let laser = stats_of(items, "VNCL_LaserCannon_S1")?;
    assert_eq!(laser["weapon"]["fireActions"][0]["fireRate"], 350.0);
    let drive = stats_of(items, "QDRV_WETK_S04_Idris_TEMP")?;
    assert!(
        drive["quantumDrive"]["jumpRange"]
            .as_f64()
            .is_some_and(|range| range > 3.4e38),
        "the stored f32::MAX"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn personal_weapons_take_ammunition_from_their_magazine() -> TestResult {
    let catalog = catalog()?;
    let items = catalog["items"].as_array().ok_or("the items array")?;
    let personal: Vec<&Value> = items
        .iter()
        .filter(|item| item["type"] == "WeaponPersonal" && present(&item["stats"]["weapon"]))
        .collect();
    let with_ammo = personal
        .iter()
        .filter(|item| {
            present(&item["stats"]["weapon"]["ammo"]["damage"])
                && item["stats"]["weapon"]["ammoCapacity"]
                    .as_i64()
                    .is_some_and(|capacity| capacity > 0)
        })
        .count();
    report(&format!(
        "personal weapons: {}, with magazine ammunition: {with_ammo}",
        personal.len()
    ));
    assert!(with_ammo > 0, "a personal weapon with magazine ammunition");
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2^52")]
    let share = with_ammo as f64 / personal.len() as f64;
    assert!(share >= MIN_SHARE, "{with_ammo} of {}", personal.len());
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn weapons_state_ammunition_damage_and_a_fire_rate() -> TestResult {
    let catalog = catalog()?;
    let items = catalog["items"].as_array().ok_or("the items array")?;
    let damaging = items
        .iter()
        .filter(|item| present(&item["stats"]["weapon"]["ammo"]["damage"]))
        .count();
    let rated = items
        .iter()
        .filter(|item| {
            item["stats"]["weapon"]["fireActions"]
                .as_array()
                .is_some_and(|actions| actions.iter().any(|action| present(&action["fireRate"])))
        })
        .count();
    report(&format!(
        "weapons with ammunition damage: {damaging}, with a fire rate: {rated}"
    ));
    assert!(damaging > 0, "a weapon with ammunition damage");
    assert!(rated > 0, "a weapon with a fire rate");
    Ok(())
}
