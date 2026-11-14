// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The mining laser, module and gadget stat blocks of the P4KX catalog.

#[path = "catalog_mining_configurations.rs"]
mod configurations;

use crate::gear_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use gear_fixture::{
    ARMOR, GADGET, GLOBALS, LASER, LASER_RANGE, LASER_TWIN, MODULE, MODULE_INSTABILITY,
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

fn guid_text(guid: u128) -> String {
    let text = format!("{guid:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
}

fn item(catalog: &Value, guid: u128) -> Result<Value, Box<dyn Error>> {
    let wanted = guid_text(guid);
    catalog
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("guid") == Some(&json!(wanted)))
        })
        .cloned()
        .ok_or_else(|| format!("item {wanted}").into())
}

fn stats(guid: u128) -> Result<Value, Box<dyn Error>> {
    Ok(item(&catalog()?, guid)?
        .get("stats")
        .cloned()
        .unwrap_or(Value::Null))
}

#[test]
fn a_laser_carries_its_throttle_and_modifiers_as_stored() -> TestResult {
    let stats = stats(LASER)?;
    assert_eq!(
        stats["miningLaser"],
        json!({
            "isOptimalRateGood": true,
            "throttleLerpSpeed": 7.0,
            "throttleMinimum": 0.2,
            "usesPowerThrottle": false,
            "globalParamsGuid": guid_text(GLOBALS),
            "modifiers": {
                "instability": 35.0,
                "optimalChargeWindowSize": 40.0,
                "resistance": 25.0,
                "shatterDamage": null,
                "clusterFactor": null,
                "optimalChargeWindowRate": -40.0,
                "catastrophicChargeWindowRate": null
            },
            "filterModifier": 40.0
        })
    );
    assert!(stats["miningModifier"].is_null(), "no attachable component");
    Ok(())
}

#[test]
fn the_beams_of_a_laser_state_their_ranges_and_hit_type() -> TestResult {
    let stats = stats(LASER)?;
    let actions = stats["weapon"]["fireActions"]
        .as_array()
        .ok_or("the fire actions")?;
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0]["hitType"], "ElectricArc");
    assert_eq!(actions[0]["fullDamageRange"], 40.0);
    assert_eq!(actions[0]["zeroDamageRange"], 45.0);
    assert_eq!(actions[0]["damagePerSecond"]["energy"], 3900.0);
    assert_eq!(actions[1]["hitType"], "Extraction");
    Ok(())
}

#[test]
fn a_module_carries_its_charges_lifetime_and_multipliers() -> TestResult {
    let stats = stats(MODULE)?;
    let block = &stats["miningModifier"];
    assert_eq!(block["activationMethod"], "ActivateOnDemand");
    assert_eq!(block["charges"], 6);
    assert_eq!(block["canInterrupt"], true);
    assert_eq!(block["isInterruptible"], false);
    assert_eq!(block["laser"]["lifetime"], 30.0);
    assert_eq!(block["laser"]["isOptimalRateGood"], true);
    assert_eq!(block["laser"]["modifiers"]["instability"], -10.0);
    assert_eq!(block["laser"]["modifiers"]["shatterDamage"], -10.0);
    assert!(block["laser"]["modifiers"]["resistance"].is_null());
    assert!(block["rock"].is_null());
    assert_eq!(
        block["filter"],
        json!({ "lifetime": 30.0, "filterModifier": null })
    );
    assert_eq!(
        block["fireActionModifiers"],
        json!([
            { "fireActionIndex": 0, "lifetime": 30.0, "damageMultiplier": 1.35 },
            { "fireActionIndex": 1, "lifetime": 30.0, "damageMultiplier": 0.85 }
        ])
    );
    assert!(stats["miningLaser"].is_null(), "no laser component");
    Ok(())
}

#[test]
fn a_gadget_carries_the_modifiers_it_applies_to_the_rock() -> TestResult {
    let stats = stats(GADGET)?;
    let block = &stats["miningModifier"];
    assert_eq!(block["charges"], -1);
    assert!(block["laser"].is_null());
    assert!(block["rock"]["lifetime"].is_null());
    assert_eq!(block["rock"]["modifiers"]["optimalChargeWindowSize"], -30.0);
    assert_eq!(block["rock"]["modifiers"]["resistance"], -25.0);
    assert_eq!(block["rock"]["modifiers"]["clusterFactor"], 60.0);
    assert_eq!(block["fireActionModifiers"], json!([]));
    Ok(())
}

#[test]
fn an_item_without_mining_components_has_no_mining_blocks() -> TestResult {
    let stats = stats(ARMOR)?;
    assert!(stats["miningLaser"].is_null());
    assert!(stats["miningModifier"].is_null());
    Ok(())
}

#[test]
fn lasers_equal_in_every_compared_field_are_variants() -> TestResult {
    let catalog = catalog()?;
    let base = item(&catalog, LASER)?["variantOf"].clone();
    let twin = item(&catalog, LASER_TWIN)?["variantOf"].clone();
    assert!(
        base == json!(guid_text(LASER_TWIN)) || twin == json!(guid_text(LASER)),
        "one of the twins is the base of the other"
    );
    Ok(())
}

#[test]
fn a_beam_range_or_a_module_modifier_starts_a_variant_group_of_its_own() -> TestResult {
    let catalog = catalog()?;
    let range = item(&catalog, LASER_RANGE)?;
    assert!(
        range["variantOf"].is_null(),
        "a different range is no variant"
    );
    for guid in [LASER, LASER_TWIN] {
        assert_ne!(
            item(&catalog, guid)?["variantOf"],
            json!(guid_text(LASER_RANGE))
        );
    }
    assert!(item(&catalog, MODULE)?["variantOf"].is_null());
    assert!(item(&catalog, MODULE_INSTABILITY)?["variantOf"].is_null());
    Ok(())
}
