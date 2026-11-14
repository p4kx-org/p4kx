// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The armour, clothing, radar, tractor beam and attachment stat blocks of the P4KX catalog.

use crate::gear_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use gear_fixture::{
    ARMOR, ARMOR_DANGLING, BEAM, CLOTHING, PARALLEL, PLAIN_MODIFIER, RADAR, SALVAGE_MODIFIER,
    TRACTOR, WRAPPED_TRACTOR,
};
use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn catalog() -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        gear_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    let built = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn stats(catalog: &Value, guid: u128) -> Result<Value, Box<dyn Error>> {
    let text = format!("{guid:032x}");
    let wanted = format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    );
    catalog["items"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["guid"] == wanted.as_str()))
        .map(|item| item["stats"].clone())
        .ok_or_else(|| format!("item {wanted}").into())
}

#[test]
fn armour_carries_the_resistances_of_its_macro_as_stored() -> TestResult {
    let stats = stats(&catalog()?, ARMOR)?;
    assert_eq!(
        stats["armor"],
        json!({
            "integrityMilestoneToBreak": 0.1,
            "damageResistance": {
                "ignoreMeleeDamage": false,
                "physical": {
                    "multiplier": 0.6, "threshold": 0.0, "damageCap": 0.0, "damageCapLimit": 0.0
                },
                "energy": {
                    "multiplier": 0.75, "threshold": 2.0, "damageCap": 0.0, "damageCapLimit": 0.0
                },
                "distortion": null,
                "thermal": null,
                "biochemical": null,
                "stun": null
            }
        })
    );
    assert!(stats["clothing"].is_null(), "no clothing component");
    Ok(())
}

#[test]
fn armour_with_a_dangling_macro_keeps_its_block_without_resistances() -> TestResult {
    let stats = stats(&catalog()?, ARMOR_DANGLING)?;
    assert_eq!(stats["armor"]["integrityMilestoneToBreak"], 0.1);
    assert!(stats["armor"]["damageResistance"].is_null());
    Ok(())
}

#[test]
fn clothing_carries_its_temperature_and_radiation_values() -> TestResult {
    let stats = stats(&catalog()?, CLOTHING)?;
    assert_eq!(
        stats["clothing"],
        json!({
            "temperatureResistanceMin": -70.0,
            "temperatureResistanceMax": 100.0,
            "radiationCapacity": 26800.0,
            "radiationDissipationRate": 145.8,
            "gForceResistance": -0.5
        })
    );
    Ok(())
}

#[test]
fn a_radar_carries_its_aim_assist_distances() -> TestResult {
    let stats = stats(&catalog()?, RADAR)?;
    assert_eq!(
        stats["radar"],
        json!({
            "aimAssistDistanceMin": 780.0,
            "aimAssistDistanceMax": 1028.5,
            "aimAssistOutsideRangeBuffer": 60.0,
            "pingCooldownTime": 2.5
        })
    );
    Ok(())
}

#[test]
fn a_tractor_beam_carries_the_values_of_its_action() -> TestResult {
    let stats = stats(&catalog()?, TRACTOR)?;
    assert_eq!(
        stats["tractorBeam"],
        json!({
            "minForce": 1500.0,
            "maxForce": 500_000.0,
            "minDistance": 0.0,
            "maxDistance": 150.0,
            "fullStrengthDistance": 75.0,
            "maxVolume": 300_000.0,
            "maxAngle": 60.0
        })
    );
    Ok(())
}

#[test]
fn a_salvage_modifier_carries_its_multipliers() -> TestResult {
    let stats = stats(&catalog()?, SALVAGE_MODIFIER)?;
    assert_eq!(
        stats["weaponModifier"],
        json!({
            "salvage": {
                "radiusMultiplier": 3.5,
                "salvageSpeedMultiplier": 0.15,
                "extractionEfficiency": 0.9
            },
            "tractor": {
                "forceMultiplier": 1.0,
                "fullStrengthDistMultiplier": 1.0,
                "maxDistMultiplier": 1.0,
                "maxVolumeMultiplier": 1.0
            }
        })
    );
    Ok(())
}

#[test]
fn an_attachment_without_salvage_or_tractor_modifier_has_no_stats() -> TestResult {
    assert!(stats(&catalog()?, PLAIN_MODIFIER)?.is_null());
    Ok(())
}

#[test]
fn a_beam_states_its_damage_per_second() -> TestResult {
    let stats = stats(&catalog()?, BEAM)?;
    let action = &stats["weapon"]["fireActions"][0];
    assert_eq!(action["kind"], "SWeaponActionFireBeamParams");
    assert_eq!(action["damagePerSecond"]["energy"], 2340.0);
    assert_eq!(action["damagePerSecond"]["physical"], 0.0);
    Ok(())
}

#[test]
fn a_tractor_action_inside_a_wrapper_next_to_another_action_is_found() -> TestResult {
    let stats = stats(&catalog()?, WRAPPED_TRACTOR)?;
    assert_eq!(stats["tractorBeam"]["maxForce"], 42.0);
    assert_eq!(stats["tractorBeam"]["maxDistance"], 7.0);
    assert_eq!(
        stats["tractorBeam"]["maxVolume"], 0.0,
        "an unset single is stored as 0"
    );
    assert_eq!(
        stats["weapon"]["fireActions"].as_array().map(Vec::len),
        Some(2),
        "the beam and the wrapper"
    );
    Ok(())
}

#[test]
fn the_actions_of_a_parallel_mode_are_listed_one_by_one() -> TestResult {
    let stats = stats(&catalog()?, PARALLEL)?;
    let actions = stats["weapon"]["fireActions"]
        .as_array()
        .ok_or("the fire actions")?;
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0]["damagePerSecond"]["energy"], 10.0);
    Ok(())
}
