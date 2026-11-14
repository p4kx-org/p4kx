// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The insurance data of the ships in the Basetool catalog, through the public API.

use crate::catalog_recovery::{legacy, overrides, recovery_row};
use crate::health_origin_assertions as proof;
use crate::insurance_fixture::{BARE, INSURED, ITEM, RECOVERY, UNINSURED, write_archive};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn build() -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    let built = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn guid(value: u128) -> String {
    let text = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
}

fn ship(catalog: &Value, value: u128) -> Result<&Value, Box<dyn Error>> {
    let wanted = guid(value);
    catalog["ships"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|ship| ship["guid"] == wanted.as_str())
        .ok_or_else(|| format!("no ship {wanted}").into())
}

#[test]
fn a_ship_carries_the_claim_and_expedite_values_of_its_record() -> TestResult {
    let catalog = build()?;
    let insured = ship(&catalog, INSURED)?;
    assert_eq!(
        insured["insurance"],
        json!({
            "restoreOnClaim": true,
            "shipEntityClassName": "AEGS_Insured",
            "baseWaitTimeMinutes": 5.83,
            "mandatoryWaitTimeMinutes": 1.94,
            "expediteFee": 2335
        })
    );
    assert_eq!(
        insured["recordPath"], "libs/foundry/records/entities/spaceships/aegs_insured",
        "the source path is the ship's record path"
    );
    Ok(())
}

#[test]
fn a_ship_without_insurance_properties_has_null() -> TestResult {
    let catalog = build()?;
    assert_eq!(ship(&catalog, UNINSURED)?["insurance"], Value::Null);
    Ok(())
}

#[test]
fn empty_values_of_a_ground_vehicle_stay_facts() -> TestResult {
    let catalog = build()?;
    assert_eq!(
        ship(&catalog, BARE)?["insurance"],
        json!({
            "restoreOnClaim": false,
            "shipEntityClassName": null,
            "baseWaitTimeMinutes": 0.0,
            "mandatoryWaitTimeMinutes": 0.0,
            "expediteFee": 0
        })
    );
    Ok(())
}

#[test]
fn the_recovery_multipliers_are_exported_with_the_classes_they_apply_to() -> TestResult {
    let catalog = build()?;
    let recovery = recovery_row(&catalog)?;
    assert_eq!(
        recovery["source"]["recordPath"]["value"],
        "libs/foundry/records/entitlementpolicies/globalitemrecovery"
    );
    assert_eq!(
        recovery["source"]["className"]["value"],
        "GlobalItemRecovery"
    );
    assert_eq!(recovery["source"]["guid"]["value"], guid(RECOVERY).as_str());
    let legacy = legacy(&catalog)?;
    assert_eq!(
        legacy["claimCostMultiplier"]["value"]["finite"].as_f64(),
        Some(0.003)
    );
    assert_eq!(
        legacy["maxExpeditedClaimCostMultiplier"]["value"]["finite"].as_f64(),
        Some(0.0003)
    );
    assert_eq!(
        legacy["expeditedDeliverySecondsPerAUEC"]["value"]["finite"].as_f64(),
        Some(5e-7)
    );
    assert_eq!(legacy["globalBrickTimer"]["state"], "absent");
    assert_eq!(
        recovery["economyParams"]["value"]["escalatingClaimsParams"]["value"]["escalatingClaimChainLimit"]
            ["value"]["value"],
        1
    );
    let overrides = proof::rows(overrides(&catalog)?, "/value")?;
    assert_eq!(overrides.len(), 2);
    let classes = &overrides[0]["value"]["classes"]["value"];
    assert_eq!(classes[0]["value"]["guid"]["value"], guid(INSURED).as_str());
    assert_eq!(classes[1]["value"]["guid"]["value"], guid(BARE).as_str());
    assert_eq!(
        overrides[0]["value"]["claimCostMultiplier"]["value"]["finite"].as_f64(),
        Some(1.0)
    );
    assert_eq!(overrides[1]["value"]["classes"]["value"], json!([]));
    assert_eq!(
        overrides[1]["value"]["claimCostMultiplier"]["value"]["finite"].as_f64(),
        Some(0.6)
    );
    Ok(())
}

#[test]
fn only_ships_carry_insurance() -> TestResult {
    let catalog = build()?;
    let wanted = guid(ITEM);
    let item = catalog["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|item| item["guid"] == wanted.as_str())
        .ok_or("the item is missing")?;
    assert!(item.get("insurance").is_none());
    Ok(())
}
