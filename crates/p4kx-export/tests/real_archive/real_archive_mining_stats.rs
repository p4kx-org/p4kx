// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::real_archive_equipment_catalog::{
    TestResult, WEAPONS, action_nodes, catalog, item, item_catalog, owner, scalar,
};
use crate::real_archive_item_durability::rows;

const LASERS: &str = "/configurations/miningLasers/rows";
const ATTACHABLE: &str = "/configurations/attachableModifiers/rows";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_ship_laser_states_its_throttle_modifiers_and_beams() -> TestResult {
    const LASER: &str = "Mining_Laser_DRAK_Golem_S1";
    let catalog = item_catalog(LASER)?;
    let item = item(&catalog, LASER)?;
    let laser = owner(&catalog, LASERS, item)?;
    scalar(laser, "/throttleLerpSpeed", 7.0)?;
    scalar(laser, "/throttleMinimum", 0.2)?;
    let modifiers = object(laser, "/miningLaserModifiers/value")?;
    modifier(modifiers, "laserInstability", 35.0)?;
    modifier(modifiers, "optimalChargeWindowRateModifier", -40.0)?;
    assert_eq!(
        modifiers["shatterdamageModifier"]["state"], "null",
        "shatter damage is unset"
    );
    scalar(
        laser,
        "/filterParams/value/filterModifier/value/value",
        40.0,
    )?;
    assert!(
        laser
            .pointer("/globalParams/value/guid/value")
            .is_some_and(Value::is_string),
        "original global configuration Reference"
    );
    let weapon = owner(&catalog, WEAPONS, item)?;
    let beams: Vec<_> = action_nodes(weapon)
        .filter_map(|node| (node["kind"]["type"] == "beam").then_some(&node["kind"]["value"]))
        .collect();
    let [mining, extraction] = beams.as_slice() else {
        return Err("mining and extraction beams".into());
    };
    assert_eq!(
        mining["hitType"]["value"], "ElectricArc",
        "the first beam mines"
    );
    scalar(mining, "/fullDamageRange", 40.0)?;
    scalar(mining, "/zeroDamageRange", 45.0)?;
    scalar(mining, "/damagePerSecond/value/value/energy", 3900.0)?;
    assert_eq!(
        extraction["hitType"]["value"], "Extraction",
        "the second beam extracts"
    );
    scalar(extraction, "/damagePerSecond/value/value/energy", 1295.0)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn an_active_module_states_its_charges_lifetime_and_multipliers() -> TestResult {
    const MODULE: &str = "mining_modules_active_stampede";
    let catalog = item_catalog(MODULE)?;
    let module = owner(&catalog, ATTACHABLE, item(&catalog, MODULE)?)?;
    assert_eq!(
        module["activationMethod"]["value"], "ActivateOnDemand",
        "active module trigger"
    );
    assert_eq!(module["charges"]["value"], 6, "active module charges");
    let laser = branch(module, "miningLaser").ok_or("active laser modifier")?;
    scalar(laser, "/modifierLifetime/value/lifetime", 30.0)?;
    let modifiers = object(laser, "/miningLaserModifier/value")?;
    modifier(modifiers, "laserInstability", -10.0)?;
    modifier(modifiers, "shatterdamageModifier", -10.0)?;
    let first = branch(module, "weapon").ok_or("first original weapon action modifier")?;
    assert_eq!(
        first["fireActionIndex"]["value"], 0,
        "the first authored fire action index"
    );
    scalar(
        first,
        "/weaponModifier/value/weaponStats/value/damageMultiplier",
        1.35,
    )
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_passive_module_and_a_gadget_state_their_modifiers() -> TestResult {
    const PASSIVE: &str = "mining_modules_passive_vaux_mk1";
    const GADGET: &str = "mining_gadget_grin_optimax";
    let passive_catalog = item_catalog(PASSIVE)?;
    let passive = owner(
        &passive_catalog,
        ATTACHABLE,
        item(&passive_catalog, PASSIVE)?,
    )?;
    assert_eq!(
        passive["activationMethod"]["value"], "ActivateOnAttach",
        "passive module trigger"
    );
    let laser = branch(passive, "miningLaser").ok_or("passive laser modifier")?;
    assert_eq!(
        laser["modifierLifetime"]["state"], "null",
        "passive modifier has no lifetime"
    );
    modifier(
        object(laser, "/miningLaserModifier/value")?,
        "optimalChargeWindowRateModifier",
        -20.0,
    )?;

    let gadget_catalog = item_catalog(GADGET)?;
    let gadget = owner(&gadget_catalog, ATTACHABLE, item(&gadget_catalog, GADGET)?)?;
    assert!(
        branch(gadget, "miningLaser").is_none(),
        "the gadget applies to the rock"
    );
    let rock = branch(gadget, "mineableRock").ok_or("gadget rock modifier")?;
    let modifiers = object(rock, "/miningLaserModifier/value")?;
    modifier(modifiers, "optimalChargeWindowSizeModifier", -30.0)?;
    modifier(modifiers, "resistanceModifier", -25.0)?;
    modifier(modifiers, "clusterFactorModifier", 60.0)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_catalog_lists_the_lasers_modules_and_gadgets_of_the_game() -> TestResult {
    let catalog = catalog()?;
    let lasers = rows(&catalog, LASERS)?.len();
    let modifiers = rows(&catalog, ATTACHABLE)?;
    let modules = modifiers
        .iter()
        .filter(|row| branch(row, "miningLaser").is_some())
        .count();
    let gadgets = modifiers
        .iter()
        .filter(|row| branch(row, "mineableRock").is_some())
        .count();
    assert!(lasers >= 18, "{lasers} mining lasers");
    assert!(modules >= 29, "{modules} modules");
    assert!(gadgets >= 6, "{gadgets} gadgets");
    Ok(())
}

fn object<'row>(row: &'row Value, path: &str) -> TestResult<&'row Value> {
    row.pointer(path)
        .filter(|value| value.is_object())
        .ok_or_else(|| format!("original object at {path}").into())
}

fn modifier(row: &Value, field: &str, expected: f32) -> TestResult {
    scalar(row, &format!("/{field}/value/value"), expected)
}

fn branch<'row>(row: &'row Value, kind: &str) -> Option<&'row Value> {
    row.pointer("/modifiers/value")?
        .as_array()?
        .iter()
        .filter_map(|slot| slot.get("value"))
        .find_map(|value| {
            (value["type"] == kind)
                .then(|| value.get("value"))
                .flatten()
        })
}
