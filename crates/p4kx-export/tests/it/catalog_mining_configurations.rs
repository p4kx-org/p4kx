// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::{Archive, Filter};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, TestResult};
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
    proof::cell(modifiers, "shatterdamageModifier", "null")?;
    single(
        proof::at(row, "/filterParams/value/filterModifier/value")?,
        "value",
        40.0,
    )?;
    let global = proof::at(&catalog, "/definitions/miningLaserGlobals/rows/0")?;
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
    single(
        proof::at(
            laser,
            "/value/value/miningLaserModifier/value/shatterdamageModifier/value",
        )?,
        "value",
        -10.0,
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
    let rock = proof::at(row, "/modifiers/value/0/value")?;
    assert_eq!(proof::at(rock, "/type")?, "mineableRock", "rock branch");
    proof::cell(proof::at(rock, "/value")?, "modifierLifetime", "null")?;
    single(
        proof::at(
            rock,
            "/value/miningLaserModifier/value/clusterFactorModifier/value",
        )?,
        "value",
        60.0,
    )?;
    proof::closure(&gadget)?;
    let salvage = catalog("*/gear/salvage_modifier")?;
    let stats = proof::at(
        &salvage,
        "/configurations/attachableModifiers/rows/0/modifiers/value/0/value/value/weaponModifier/value/weaponStats/value",
    )?;
    single(
        proof::at(stats, "/salvageModifier/value")?,
        "radiusMultiplier",
        3.5,
    )?;
    single(
        proof::at(stats, "/tractorModifier/value")?,
        "forceMultiplier",
        1.0,
    )?;
    proof::closure(&salvage)
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
