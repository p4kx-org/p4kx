// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The gear stat blocks (armour, clothing, radar, tractor beam, salvage modifier, mining laser) and
//! the trace of every gameplay property to a stat field, on a real Archive; runs with `--ignored`
//! when `P4KX_DATA_P4K` names one.

use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Kind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use crate::catalog_v3_consumer;

type TestResult = Result<(), Box<dyn Error>>;

const DOCS: &str = include_str!("../../../../docs/basetool-catalog.md");

#[expect(clippy::print_stdout, reason = "the numbers are the result of the run")]
fn report(line: &str) {
    println!("{line}");
}

fn catalog() -> Result<Value, Box<dyn Error>> {
    let archive = Archive::open_with_key(real_archive_path()?, None)?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    Ok(serde_json::from_slice(built.json())?)
}

fn item<'c>(catalog: &'c Value, class: &str) -> Result<&'c Value, String> {
    catalog["items"]
        .as_array()
        .and_then(|items| {
            items.iter().find(|item| {
                item["className"]
                    .as_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case(class))
            })
        })
        .ok_or_else(|| format!("no item {class}"))
}

fn show(label: &str, block: &Value) {
    report(&format!("{label}: {block}"));
}

fn count(catalog: &Value, block: &str) -> usize {
    catalog["items"].as_array().map_or(0, |items| {
        items
            .iter()
            .filter(|item| {
                item.get("stats")
                    .and_then(|stats| stats.get(block))
                    .is_some_and(|value| !value.is_null())
            })
            .count()
    })
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn an_armour_piece_states_its_resistances_and_its_worn_values() -> TestResult {
    let catalog = catalog()?;
    let armour = item(&catalog, "cds_armor_heavy_core_01_01_01")?;
    show("armor", &armour["stats"]["armor"]);
    show("clothing", &armour["stats"]["clothing"]);
    report(&format!(
        "armor blocks: {}, clothing blocks: {}",
        count(&catalog, "armor"),
        count(&catalog, "clothing")
    ));
    let physical = &armour["stats"]["armor"]["damageResistance"]["physical"];
    assert_eq!(physical["multiplier"], 0.6);
    assert_eq!(physical["threshold"], 0.0);
    assert_eq!(armour["stats"]["armor"]["integrityMilestoneToBreak"], 0.1);
    let clothing = &armour["stats"]["clothing"];
    assert_eq!(clothing["temperatureResistanceMin"], -70.0);
    assert_eq!(clothing["temperatureResistanceMax"], 100.0);
    assert_eq!(clothing["radiationCapacity"], 26800.0);
    assert_eq!(clothing["radiationDissipationRate"], 145.8);
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_clothing_piece_states_its_temperature_range() -> TestResult {
    let catalog = catalog()?;
    let shoes = item(&catalog, "987_shoes_01_01_01")?;
    show("clothing", &shoes["stats"]["clothing"]);
    show("armor", &shoes["stats"]["armor"]);
    assert_eq!(shoes["stats"]["clothing"]["temperatureResistanceMax"], 25.0);
    assert!(
        shoes["stats"]["clothing"]["temperatureResistanceMin"].is_number(),
        "a minimum temperature"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_radar_states_its_aim_assist_distances() -> TestResult {
    let catalog = catalog()?;
    let radar = item(&catalog, "radr_bltr_s01_hunter")?;
    show("radar", &radar["stats"]["radar"]);
    report(&format!("radar blocks: {}", count(&catalog, "radar")));
    assert_eq!(radar["stats"]["radar"]["aimAssistDistanceMin"], 780.0);
    assert_eq!(radar["stats"]["radar"]["aimAssistDistanceMax"], 1028.5);
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_tractor_beam_states_force_distances_and_volume() -> TestResult {
    let catalog = catalog()?;
    let beam = item(&catalog, "grin_tractorbeam_s1")?;
    show("tractorBeam", &beam["stats"]["tractorBeam"]);
    report(&format!(
        "tractor beams: {}",
        count(&catalog, "tractorBeam")
    ));
    let block = &beam["stats"]["tractorBeam"];
    assert_eq!(block["maxForce"], 500_000.0);
    assert_eq!(block["maxDistance"], 150.0);
    assert_eq!(block["fullStrengthDistance"], 75.0);
    assert_eq!(block["maxVolume"], 300_000.0);
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_salvage_modifier_states_its_multipliers() -> TestResult {
    let catalog = catalog()?;
    let scraper = item(&catalog, "salvage_modifier_scraper_medium")?;
    show("weaponModifier", &scraper["stats"]["weaponModifier"]);
    report(&format!(
        "weapon modifiers: {}",
        count(&catalog, "weaponModifier")
    ));
    let salvage = &scraper["stats"]["weaponModifier"]["salvage"];
    assert_eq!(salvage["salvageSpeedMultiplier"], 0.15);
    assert_eq!(salvage["radiusMultiplier"], 3.5);
    assert_eq!(salvage["extractionEfficiency"], 0.9);
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_mining_laser_states_its_damage_per_second() -> TestResult {
    let catalog = catalog()?;
    let laser = item(&catalog, "mining_laser_grin_arbor_s1")?;
    show("weapon", &laser["stats"]["weapon"]["fireActions"]);
    let power = laser["stats"]["weapon"]["fireActions"]
        .as_array()
        .and_then(|actions| {
            actions
                .iter()
                .find_map(|action| action["damagePerSecond"]["energy"].as_f64())
        });
    assert_eq!(power, Some(2340.0));
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_gameplay_property_is_traced_to_a_stat_field_or_listed_as_none() -> TestResult {
    let catalog = catalog()?;
    let properties = catalog_v3_consumer::rows(&catalog, Kind::GameplayProperties)?;
    let mut untraced = Vec::new();
    for property in properties {
        let class = catalog_v3_consumer::field(property, Kind::GameplayProperties, "className")?
            .as_str()
            .ok_or("a present className")?;
        let row = DOCS
            .lines()
            .find(|line| line.starts_with("| `") && line.contains(&format!("`{class}`")));
        report(&format!(
            "{class}: {}",
            row.unwrap_or("NOT IN THE DOCUMENT")
        ));
        if row.is_none() {
            untraced.push(class.to_owned());
        }
    }
    assert!(
        untraced.is_empty(),
        "properties not in the table: {untraced:?}"
    );
    report(&format!("{} properties traced", properties.len()));
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn fire_modes_state_their_pellets() -> TestResult {
    let catalog = catalog()?;
    let items = catalog["items"].as_array().ok_or("the items")?;
    let mut modes = 0_usize;
    let mut with_pellets = 0_usize;
    for item in items {
        let Some(actions) = item["stats"]["weapon"]["fireActions"].as_array() else {
            continue;
        };
        for action in actions {
            modes += 1;
            if action["pellets"].as_i64().is_some_and(|count| count > 0) {
                with_pellets += 1;
            }
        }
    }
    report(&format!(
        "fire modes: {modes}, with pellets: {with_pellets}"
    ));
    assert!(with_pellets > 0, "a fire mode with pellets");
    Ok(())
}
