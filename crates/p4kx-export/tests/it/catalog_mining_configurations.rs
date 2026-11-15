// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::{Archive, Filter};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::gear_fixture::{GLOBALS, LASER};
use crate::health_origin_assertions as proof;

#[test]
fn a_filtered_laser_keeps_its_original_modifiers_and_resolved_global_record() -> TestResult {
    let catalog = catalog("*/gear/mining_laser")?;
    let row = proof::at(&catalog, "/configurations/miningLasers/rows/0")?;
    proof::present(row, "componentOrdinal", &json!(2), Some(true))?;
    single(row, "throttleLerpSpeed", 7.0)?;
    single(row, "throttleMinimum", 0.2)?;
    proof::present(row, "usesPowerThrottle", &json!(false), Some(true))?;
    let modifiers = proof::at(row, "/miningLaserModifiers/value")?;
    proof::present(modifiers, "isOptimalRateGood", &json!(true), Some(true))?;
    let instability = proof::at(modifiers, "/laserInstability/value")?;
    proof::present(
        instability,
        "sourceType",
        &json!("FloatModifierMultiplicative"),
        Some(true),
    )?;
    single(instability, "value", 35.0)?;
    laser_modifier(modifiers, "optimalChargeWindowSizeModifier", 40.0)?;
    laser_modifier(modifiers, "resistanceModifier", 25.0)?;
    laser_modifier(modifiers, "optimalChargeWindowRateModifier", -40.0)?;
    for field in [
        "shatterdamageModifier",
        "clusterFactorModifier",
        "catastrophicChargeWindowRateModifier",
    ] {
        proof::cell(modifiers, field, "null")?;
    }
    no_mining_rows(&catalog, "attachableModifiers")?;
    single(
        proof::at(row, "/filterParams/value/filterModifier/value")?,
        "value",
        40.0,
    )?;
    let global = proof::at(&catalog, "/definitions/miningLaserGlobals/rows/0")?;
    proof::present(
        proof::at(global, "/source")?,
        "guid",
        &json!(common::guid(GLOBALS)),
        Some(true),
    )?;
    assert_eq!(
        proof::at(row, "/globalParams/value/targetRecordIndex/value")?,
        proof::at(global, "/source/recordIndex/value")?,
        "selected laser link identifies the original matched global Record",
    );
    single(global, "throttleChangePerAction", 0.25)?;
    proof::present(
        global,
        "blockThrottleChangeWhenNotFiring",
        &json!(true),
        Some(true),
    )?;
    proof::closure(&catalog)
}

#[test]
fn attachable_modifiers_keep_laser_filter_and_fire_action_slots_in_order() -> TestResult {
    let catalog = catalog("*/gear/mining_module")?;
    let row = proof::at(&catalog, "/configurations/attachableModifiers/rows/0")?;
    proof::present(row, "charges", &json!(6), Some(true))?;
    proof::present(row, "canInterrupt", &json!(true), Some(true))?;
    proof::present(row, "isInterruptible", &json!(false), Some(true))?;
    no_mining_rows(&catalog, "miningLasers")?;
    proof::present(
        row,
        "activationMethod",
        &json!("ActivateOnDemand"),
        Some(true),
    )?;
    let slots = proof::rows(row, "/modifiers/value")?;
    let [first, second, laser, filter, booster] = slots else {
        return Err("five ordered original modifiers".into());
    };
    weapon(first, 0, 1.35)?;
    weapon(second, 1, 0.85)?;
    assert_eq!(
        proof::at(laser, "/value/type")?,
        "miningLaser",
        "laser branch"
    );
    single(
        proof::at(laser, "/value/value/modifierLifetime/value")?,
        "lifetime",
        30.0,
    )?;
    let laser_modifiers = proof::at(laser, "/value/value/miningLaserModifier/value")?;
    proof::present(
        laser_modifiers,
        "isOptimalRateGood",
        &json!(true),
        Some(true),
    )?;
    laser_modifier(laser_modifiers, "laserInstability", -10.0)?;
    laser_modifier(laser_modifiers, "shatterdamageModifier", -10.0)?;
    proof::cell(laser_modifiers, "resistanceModifier", "null")?;
    single(
        proof::at(filter, "/value/value/modifierLifetime/value")?,
        "lifetime",
        30.0,
    )?;
    assert_eq!(
        proof::at(filter, "/value/type")?,
        "miningFilter",
        "filter branch"
    );
    proof::cell(
        proof::at(filter, "/value/value/filterParams/value")?,
        "filterModifier",
        "null",
    )?;
    authored_booster(booster)?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

#[test]
fn gadget_and_salvage_keep_their_different_authored_effects() -> TestResult {
    let gadget = catalog("*/gear/mining_gadget")?;
    let row = proof::at(&gadget, "/configurations/attachableModifiers/rows/0")?;
    proof::present(row, "charges", &json!(-1), Some(true))?;
    let [slot] = proof::rows(row, "/modifiers/value")? else {
        return Err("the gadget's single original rock slot".into());
    };
    let rock = proof::at(slot, "/value")?;
    assert_eq!(proof::at(rock, "/type")?, "mineableRock", "rock branch");
    proof::cell(proof::at(rock, "/value")?, "modifierLifetime", "null")?;
    let modifiers = proof::at(rock, "/value/miningLaserModifier/value")?;
    laser_modifier(modifiers, "optimalChargeWindowSizeModifier", -30.0)?;
    laser_modifier(modifiers, "resistanceModifier", -25.0)?;
    laser_modifier(modifiers, "clusterFactorModifier", 60.0)?;
    proof::closure(&gadget)?;
    let salvage = catalog("*/gear/salvage_modifier")?;
    let stats = proof::at(
        &salvage,
        "/configurations/attachableModifiers/rows/0/modifiers/value/0/value/value/weaponModifier/value/weaponStats/value",
    )?;
    let salvage_modifier = proof::at(stats, "/salvageModifier/value")?;
    single(salvage_modifier, "radiusMultiplier", 3.5)?;
    single(salvage_modifier, "salvageSpeedMultiplier", 0.15)?;
    single(salvage_modifier, "extractionEfficiency", 0.9)?;
    let tractor_modifier = proof::at(stats, "/tractorModifier/value")?;
    for field in [
        "forceMultiplier",
        "fullStrengthDistMultiplier",
        "maxDistMultiplier",
        "maxVolumeMultiplier",
    ] {
        single(tractor_modifier, field, 1.0)?;
    }
    proof::closure(&salvage)
}

#[test]
fn the_beams_of_a_laser_keep_both_slots_and_their_original_ranges() -> TestResult {
    let catalog = catalog("*/gear/mining_laser")?;
    let owner = proof::at(&catalog, "/configurations/weaponActions/rows/0")?;
    proof::present(owner, "ownerGuid", &json!(common::guid(LASER)), Some(true))?;
    let [first_slot, second_slot] = proof::rows(owner, "/fireActions/value")? else {
        return Err("both original beam slots".into());
    };
    assert_eq!(proof::at(first_slot, "/value")?, 0);
    assert_eq!(proof::at(second_slot, "/value")?, 1);
    let [first, second] = proof::rows(owner, "/actionNodes")? else {
        return Err("both original beam action nodes".into());
    };
    let first = proof::at(first, "/kind/value")?;
    proof::present(first, "hitType", &json!("ElectricArc"), Some(true))?;
    single(first, "fullDamageRange", 40.0)?;
    single(first, "zeroDamageRange", 45.0)?;
    single(
        proof::at(first, "/damagePerSecond/value/value")?,
        "energy",
        3900.0,
    )?;
    let second = proof::at(second, "/kind/value")?;
    proof::present(second, "hitType", &json!("Extraction"), Some(true))?;
    proof::closure(&catalog)
}

#[test]
fn an_owner_without_mining_components_has_no_mining_configuration_rows() -> TestResult {
    let catalog = catalog("*/gear/armor")?;
    assert_eq!(
        proof::rows(&catalog, "/configurations/suitArmor/rows")?.len(),
        1
    );
    no_mining_rows(&catalog, "miningLasers")?;
    no_mining_rows(&catalog, "attachableModifiers")
}

fn no_mining_rows(catalog: &Value, table: &str) -> TestResult {
    assert_eq!(
        proof::at(catalog, &format!("/configurations/{table}/rows"))?,
        &json!([]),
        "no physical mining row in {table}"
    );
    Ok(())
}

fn laser_modifier(owner: &Value, field: &str, expected: f32) -> TestResult {
    single(
        proof::at(owner, &format!("/{field}/value"))?,
        "value",
        expected,
    )
}

fn authored_booster(booster: &Value) -> TestResult {
    let authored = proof::at(booster, "/value/value")?;
    assert_eq!(
        proof::at(booster, "/value/type")?,
        "authored",
        "full original branch"
    );
    proof::present(
        authored,
        "sourceType",
        &json!("ItemMiningBoosterParams"),
        Some(true),
    )?;
    let root = proof::at(authored, "/additionalFields/roots/0")?;
    proof::present(root, "name", &json!("powerLevelChange"), Some(true))?;
    let value = proof::at(booster, "/value/value/additionalFields/values/0/value")?;
    assert_eq!(
        proof::at(value, "/value/value/rawBits")?,
        "40000000",
        "normal publication retains the original Booster Single bits",
    );
    assert_eq!(
        proof::at(value, "/containsEncryptedSource")?,
        true,
        "raw-only values retain their original marker",
    );
    Ok(())
}

fn weapon(cell: &Value, index: i32, multiplier: f32) -> TestResult {
    assert_eq!(
        proof::at(cell, "/value/type")?,
        "weapon",
        "fire action modifier branch"
    );
    let weapon = proof::at(cell, "/value/value")?;
    proof::present(weapon, "fireActionIndex", &json!(index), Some(true))?;
    single(
        proof::at(weapon, "/modifierLifetime/value")?,
        "lifetime",
        30.0,
    )?;
    single(
        proof::at(weapon, "/weaponModifier/value/weaponStats/value")?,
        "damageMultiplier",
        multiplier,
    )
}

fn single(owner: &Value, field: &str, value: f32) -> TestResult {
    let finite: Value = serde_json::from_str(&serde_json::to_string(&value)?)?;
    proof::present(
        owner,
        field,
        &json!({"rawBits": format!("{:08x}", value.to_bits()), "finite": finite}),
        Some(true),
    )?;
    Ok(())
}

fn catalog(pattern: &str) -> TestResult<Value> {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        crate::gear_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let filter = Filter::parse(pattern)?;
    let mut options = common::options();
    options.filter = Some(&filter);
    let built = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}
