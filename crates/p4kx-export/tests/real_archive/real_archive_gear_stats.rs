// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Physical gear source fields and their documented gameplay property mapping.

use p4kx_export::Kind;
use serde_json::Value;

use crate::catalog_v3_consumer;
use crate::real_archive_equipment_catalog::{
    TestResult, WEAPONS, action_nodes, catalog, definition, item, item_catalog, owner, scalar,
};
use crate::real_archive_item_durability::rows;

const DOCS: &str = include_str!("../../../../docs/catalog.md");

#[expect(clippy::print_stdout, reason = "the numbers are the result of the run")]
fn report(line: &str) {
    println!("{line}");
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn an_armour_piece_states_its_resistances_and_its_worn_values() -> TestResult {
    const ARMOR: &str = "cds_armor_heavy_core_01_01_01";
    let catalog = item_catalog(ARMOR)?;
    let armour = item(&catalog, ARMOR)?;
    let suit = owner(&catalog, "/configurations/suitArmor/rows", armour)?;
    scalar(suit, "/integrityMilestoneToBreak", 0.1)?;
    let target = suit
        .pointer("/damageResistance/value/targetRecordIndex/value")
        .ok_or("an original damage resistance target")?;
    let macro_row = definition(
        &catalog,
        "/definitions/damageResistanceMacros/rows",
        target,
        "/source/recordIndex/value",
    )
    .ok_or("the exact physical macro definition")?;
    scalar(
        macro_row,
        "/damageResistance/value/physical/value/multiplier",
        0.6,
    )?;
    scalar(
        macro_row,
        "/damageResistance/value/physical/value/threshold",
        0.0,
    )?;
    let clothing = owner(&catalog, "/configurations/clothing/rows", armour)?;
    scalar(
        clothing,
        "/temperatureResistance/value/minResistance",
        -70.0,
    )?;
    scalar(
        clothing,
        "/temperatureResistance/value/maxResistance",
        100.0,
    )?;
    scalar(
        clothing,
        "/radiationResistance/value/maximumRadiationCapacity",
        26800.0,
    )?;
    scalar(
        clothing,
        "/radiationResistance/value/radiationDissipationRate",
        145.8,
    )
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_clothing_piece_states_its_temperature_range() -> TestResult {
    const SHOES: &str = "987_shoes_01_01_01";
    let catalog = item_catalog(SHOES)?;
    let shoes = owner(
        &catalog,
        "/configurations/clothing/rows",
        item(&catalog, SHOES)?,
    )?;
    scalar(shoes, "/temperatureResistance/value/maxResistance", 25.0)?;
    assert!(
        shoes
            .pointer("/temperatureResistance/value/minResistance/value/finite")
            .is_some_and(Value::is_number),
        "a readable original minimum temperature"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_radar_states_its_aim_assist_distances() -> TestResult {
    const RADAR: &str = "radr_bltr_s01_hunter";
    let catalog = item_catalog(RADAR)?;
    let radar = owner(
        &catalog,
        "/configurations/itemRadars/rows",
        item(&catalog, RADAR)?,
    )?;
    scalar(radar, "/aimAssist/value/distanceMinAssignment", 780.0)?;
    scalar(radar, "/aimAssist/value/distanceMaxAssignment", 1028.5)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_tractor_beam_states_force_distances_and_volume() -> TestResult {
    const TRACTOR: &str = "grin_tractorbeam_s1";
    let catalog = item_catalog(TRACTOR)?;
    let weapon = owner(&catalog, WEAPONS, item(&catalog, TRACTOR)?)?;
    let action = action_nodes(weapon)
        .find(|node| node["kind"]["type"] == "tractorBeam")
        .ok_or("original tractor beam action")?;
    let beam = &action["kind"]["value"];
    scalar(beam, "/maxForce", 500_000.0)?;
    scalar(beam, "/maxDistance", 150.0)?;
    scalar(beam, "/fullStrengthDistance", 75.0)?;
    scalar(beam, "/maxVolume", 300_000.0)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_salvage_modifier_states_its_multipliers() -> TestResult {
    const SCRAPER: &str = "salvage_modifier_scraper_medium";
    let catalog = item_catalog(SCRAPER)?;
    let scraper = owner(
        &catalog,
        "/configurations/attachableModifiers/rows",
        item(&catalog, SCRAPER)?,
    )?;
    let salvage = rows(scraper, "/modifiers/value")?
        .iter()
        .find_map(|row| {
            row.pointer("/value/value/weaponModifier/value/weaponStats/value/salvageModifier/value")
                .filter(|value| value.is_object())
        })
        .ok_or("authored salvage settings")?;
    scalar(salvage, "/salvageSpeedMultiplier", 0.15)?;
    scalar(salvage, "/radiusMultiplier", 3.5)?;
    scalar(salvage, "/extractionEfficiency", 0.9)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_mining_laser_states_its_damage_per_second() -> TestResult {
    const LASER: &str = "mining_laser_grin_arbor_s1";
    let catalog = item_catalog(LASER)?;
    let weapon = owner(&catalog, WEAPONS, item(&catalog, LASER)?)?;
    let beam = action_nodes(weapon)
        .find_map(|node| {
            node.pointer("/kind/value/damagePerSecond/value/value/energy")
                .filter(|cell| cell["state"] == "present")
        })
        .ok_or("original energy damage rate")?;
    assert_eq!(
        beam["value"]["finite"], 2340.0,
        "the first authored beam damage rate"
    );
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
    let weapons = rows(&catalog, WEAPONS)?;
    let modes = weapons.iter().flat_map(action_nodes).count();
    let with_pellets = weapons
        .iter()
        .flat_map(action_nodes)
        .filter(|action| {
            action
                .pointer("/kind/value/launchParams/value/pelletCount/value")
                .and_then(Value::as_i64)
                .is_some_and(|count| count > 0)
        })
        .count();
    report(&format!(
        "fire modes: {modes}, with pellets: {with_pellets}"
    ));
    assert!(with_pellets > 0, "a fire mode with pellets");
    Ok(())
}
