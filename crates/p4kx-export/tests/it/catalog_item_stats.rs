// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The typed stat blocks of items in the Basetool catalog, through the public API.

use crate::item_stats_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use item_stats_fixture::{
    COOLER, CYCLIC_AMMO_GUN, DANGLING_GUN, DRIVE, GUN, MISSILE, PERSONAL_GUN, PLAIN, PLANT, SHIELD,
    UNBOUNDED_DRIVE, WRONG_TYPE_GUN,
};
use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[path = "catalog_ammo_containers.rs"]
mod ammo_containers;
#[path = "catalog_ammo_definitions.rs"]
mod ammo_definitions;
#[path = "catalog_item_ammo_source.rs"]
mod ammo_source;
#[path = "catalog_durability.rs"]
mod durability;
#[path = "catalog_fuel_capacity.rs"]
mod fuel_capacity;
#[path = "catalog_quantum.rs"]
mod quantum;
#[path = "catalog_resource_components.rs"]
mod resource_components;
#[path = "catalog_resource_connections.rs"]
mod resource_connections;
#[path = "catalog_resource_network.rs"]
mod resource_network;
#[path = "catalog_shield.rs"]
mod shield;
#[path = "catalog_vehicle_armor.rs"]
mod vehicle_armor;

fn catalog() -> Result<Value, Box<dyn Error>> {
    let (_directory, archive) = ammo_source::archive(ammo_source::AmmoSource::CyclicMembers)?;
    catalog_from(&archive)
}

fn catalog_from(archive: &Archive) -> Result<Value, Box<dyn Error>> {
    let options = CatalogOptions {
        publication_mode: p4kx_export::CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
    };
    let built = build_catalog(archive, &options)?;
    assert!(
        !built.summary().export_health().is_complete(),
        "the exact source errors remain visible independently of readable stats"
    );
    Ok(serde_json::from_slice(built.json())?)
}

fn item(catalog: &Value, guid: u128) -> Result<Value, Box<dyn Error>> {
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
        .cloned()
        .ok_or_else(|| format!("item {wanted}").into())
}

#[test]
fn a_weapon_carries_its_fire_modes_and_ammunition_as_stored() -> TestResult {
    let gun = item(&catalog()?, GUN)?;
    let weapon = &gun["stats"]["weapon"];
    assert_eq!(
        weapon["fireActions"],
        json!([
            {
                "kind": "SWeaponActionFireRapidParams",
                "name": "rapid",
                "fireRate": 600.0,
                "pellets": 1,
                "ammoCost": 1,
                "heatPerShot": 0.5,
                "chargeTime": null,
                "overchargeTime": null,
                "cooldownTime": null,
                "hitType": null,
                "fullDamageRange": null,
                "zeroDamageRange": null,
                "damagePerSecond": null,
                "spread": {"min": 0.18, "max": 5.0, "firstAttack": 1.35, "attack": 1.25, "decay": 6.5}
            },
            {
                "kind": "SWeaponActionFireChargedParams",
                "name": "rapid",
                "fireRate": 120.0,
                "pellets": 1,
                "ammoCost": 1,
                "heatPerShot": 0.5,
                "chargeTime": 1.5,
                "overchargeTime": 3.0,
                "cooldownTime": 0.25,
                "hitType": null,
                "fullDamageRange": null,
                "zeroDamageRange": null,
                "damagePerSecond": null,
                "spread": {"min": 0.18, "max": 5.0, "firstAttack": 1.35, "attack": 1.25, "decay": 6.5}
            }
        ])
    );
    assert_eq!(weapon["ammoCapacity"], 200);
    assert_eq!(
        weapon["ammo"],
        json!({
            "speed": 1100.0,
            "lifetime": 2.5,
            "damage": {
                "physical": 30.0, "energy": 2.0, "distortion": null,
                "thermal": null, "biochemical": null, "stun": null
            },
            "detonationDamage": {
                "physical": 7.0, "energy": 0.0, "distortion": null,
                "thermal": null, "biochemical": null, "stun": null
            },
            "detonationMinRadius": 1.0,
            "detonationMaxRadius": 4.0
        })
    );
    Ok(())
}

#[test]
fn a_weapon_whose_ammunition_record_is_missing_has_null_ammo() -> TestResult {
    let gun = item(&catalog()?, DANGLING_GUN)?;
    assert!(gun["stats"]["weapon"]["ammo"].is_null());
    assert_eq!(gun["stats"]["weapon"]["ammoCapacity"], 200);
    Ok(())
}

#[test]
fn a_weapon_with_an_ammunition_record_of_another_struct_keeps_the_item_and_has_null_ammo()
-> TestResult {
    let gun = item(&catalog()?, WRONG_TYPE_GUN)?;
    assert!(gun["stats"]["weapon"]["ammo"].is_null());
    assert_eq!(gun["stats"]["weapon"]["ammoCapacity"], 200);
    Ok(())
}

#[test]
fn a_weapon_keeps_readable_ammo_fields_beside_exact_inline_cycle_errors() -> TestResult {
    let gun = item(&catalog()?, CYCLIC_AMMO_GUN)?;
    assert_eq!(
        gun["stats"]["weapon"]["ammo"],
        json!({
            "speed": 0.0,
            "lifetime": 0.0,
            "damage": null,
            "detonationDamage": null,
            "detonationMinRadius": null,
            "detonationMaxRadius": null
        })
    );
    assert_eq!(gun["stats"]["weapon"]["ammoCapacity"], 200);
    Ok(())
}

#[test]
fn a_weapon_whose_ammo_layout_genuinely_cannot_be_built_keeps_the_item_and_null_ammo() -> TestResult
{
    let (_directory, archive) = ammo_source::archive(ammo_source::AmmoSource::InvalidLayout)?;
    let gun = item(&catalog_from(&archive)?, CYCLIC_AMMO_GUN)?;
    assert!(gun["stats"]["weapon"]["ammo"].is_null());
    assert_eq!(gun["stats"]["weapon"]["ammoCapacity"], 200);
    Ok(())
}

#[test]
fn a_personal_weapon_takes_capacity_and_ammunition_from_its_magazine() -> TestResult {
    let gun = item(&catalog()?, PERSONAL_GUN)?;
    let weapon = &gun["stats"]["weapon"];
    assert_eq!(weapon["ammoCapacity"], 30);
    assert_eq!(weapon["ammo"]["speed"], 1100.0);
    assert_eq!(weapon["ammo"]["damage"]["physical"], 30.0);
    Ok(())
}

#[test]
fn an_unbounded_jump_range_is_exported_as_stored() -> TestResult {
    let catalog = catalog()?;
    let row = quantum_drive(&catalog, UNBOUNDED_DRIVE)?;
    assert!(
        row.pointer("/jumpRange/value/finite")
            .and_then(Value::as_f64)
            .is_some_and(|range| range > 3.4e38),
        "the stored f32::MAX"
    );
    Ok(())
}

#[test]
fn a_shield_carries_its_values_and_its_health() -> TestResult {
    let catalog = catalog()?;
    let shield = item(&catalog, SHIELD)?;
    let stats = &shield["stats"];
    let rows = catalog
        .pointer("/configurations/shieldGenerators/rows")
        .and_then(Value::as_array)
        .ok_or("canonical shield generators")?;
    let generator = rows
        .iter()
        .find(|row| row.pointer("/ownerGuid/value") == shield.get("guid"))
        .ok_or("shield generator for the selected item")?;
    for (field, value) in [
        ("maxHealth", 8000.0),
        ("maxRegen", 160.0),
        ("decayRatio", 0.25),
        ("downedRegenDelay", 5.0),
        ("damagedRegenDelay", 2.5),
    ] {
        assert_eq!(
            generator.pointer(&format!("/{field}/value/finite")),
            Some(&json!(value)),
            "canonical shield field {field} keeps its meaning"
        );
    }
    assert!(
        stats.get("shield").is_none(),
        "one canonical shield wire location"
    );
    assert!(stats.get("power").is_none() && stats.get("heat").is_none());
    assert_eq!(stats["health"], 450.0);
    assert!(stats["weapon"].is_null() && stats["missile"].is_null());
    Ok(())
}

#[test]
fn a_quantum_drive_carries_its_range_and_drive_parameters() -> TestResult {
    let catalog = catalog()?;
    let drive = item(&catalog, DRIVE)?;
    assert!(
        drive.pointer("/stats/quantumDrive").is_none(),
        "one canonical quantum drive wire location"
    );
    let row = quantum_drive(&catalog, DRIVE)?;
    for (path, expected) in [
        ("/fuelRequirement/value/finite", 0.5),
        ("/jumpRange/value/finite", 5.0e10),
        ("/disconnectRange/value/finite", 20_000.0),
        ("/standard/value/driveSpeed/value/finite", 2.0e8),
        ("/standard/value/cooldownTime/value/finite", 6.0),
        ("/standard/value/spoolUpTime/value/finite", 10.0),
        ("/standard/value/stageOneAccelRate/value/finite", 1_000.0),
        ("/standard/value/stageTwoAccelRate/value/finite", 2_000.0),
        ("/standard/value/engageSpeed/value/finite", 20.0),
    ] {
        assert_eq!(
            row.pointer(path),
            Some(&json!(expected)),
            "canonical quantum field {path}"
        );
    }
    Ok(())
}

fn quantum_drive(catalog: &Value, guid: u128) -> Result<&Value, Box<dyn Error>> {
    let item = item(catalog, guid)?;
    catalog
        .pointer("/configurations/quantumDrives/rows")
        .and_then(Value::as_array)
        .ok_or("canonical quantum drive rows")?
        .iter()
        .find(|row| row.pointer("/ownerGuid/value") == item.get("guid"))
        .ok_or_else(|| "canonical quantum drive of the selected item".into())
}

#[test]
fn a_power_plant_and_a_cooler_carry_the_rate_of_their_own_resource() -> TestResult {
    let catalog = catalog()?;
    for (owner, resource, unit, field, amount) in [
        (PLANT, "Power", "SPowerSegmentResourceUnit", "units", 12.0),
        (
            COOLER,
            "Coolant",
            "SStandardResourceUnit",
            "standardResourceUnits",
            40.0,
        ),
    ] {
        let item = item(&catalog, owner)?;
        assert!(
            item["stats"].is_null(),
            "only the canonical table publishes resource fields"
        );
        let row = resource_components::component(&catalog, owner)?;
        let state = &row["states"]["value"][0]["value"];
        let generated = &state["deltas"]["value"][0]["value"]["generation"]["value"];
        let stored_unit = &generated["resourceAmountPerSecond"]["value"];
        assert_eq!(generated["resource"]["value"], resource);
        assert_eq!(stored_unit["sourceType"]["value"], unit);
        assert_eq!(stored_unit[field]["value"]["finite"], amount);
    }
    let row = resource_components::component(&catalog, COOLER)?;
    let state = &row["states"]["value"][0]["value"];
    assert_eq!(state["name"]["value"], "Online");
    assert_eq!(
        state.pointer("/signatureParams/value/EMSignature/value/nominalSignature/value/finite"),
        Some(&json!(500.0))
    );
    assert_eq!(
        state.pointer("/signatureParams/value/IRSignature/value/nominalSignature/value/finite"),
        Some(&json!(80.0))
    );
    let delta = &state["deltas"]["value"][0]["value"];
    assert_eq!(delta["kind"], "conversion");
    assert_eq!(
        delta.pointer("/consumption/value/resourceAmountPerSecond/value/units/value/finite"),
        Some(&json!(2.0))
    );
    Ok(())
}

#[test]
fn a_missile_carries_its_explosion_and_targeting() -> TestResult {
    let missile = item(&catalog()?, MISSILE)?;
    assert_eq!(
        missile["stats"]["missile"],
        json!({
            "damage": {
                "physical": 1500.0, "energy": 0.0, "distortion": null,
                "thermal": null, "biochemical": null, "stun": null
            },
            "explosionMinRadius": 2.0,
            "explosionMaxRadius": 12.0,
            "trackingSignalType": "Infrared",
            "lockTime": 2.0,
            "lockingAngle": 45.0,
            "lockRangeMin": 100.0,
            "lockRangeMax": 5000.0,
            "linearSpeed": 300.0
        })
    );
    Ok(())
}

#[test]
fn an_item_without_a_stat_block_has_null_stats_whatever_connections_it_has() -> TestResult {
    let plain = item(&catalog()?, PLAIN)?;
    assert!(plain["stats"].is_null());
    Ok(())
}

#[path = "catalog_resource_topology.rs"]
mod resource_topology;
