// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Physical item configuration checks through the public catalog API.

use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::health_origin_assertions as proof;
use crate::item_stats_fixture;

use std::time::{Duration, UNIX_EPOCH};

use item_stats_fixture::{COOLER, DRIVE, MISSILE, PLAIN, PLANT, SHIELD, UNBOUNDED_DRIVE};
use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

#[path = "catalog_item_canonical_weapons.rs"]
mod canonical_weapons;

#[path = "catalog_ammo_containers.rs"]
mod ammo_containers;
#[path = "catalog_ammo_definitions.rs"]
mod ammo_definitions;
#[path = "catalog_item_ammo_source.rs"]
mod ammo_source;
#[path = "catalog_facility_cargo_grid.rs"]
mod cargo_grid;
#[path = "catalog_durability.rs"]
mod durability;
#[path = "catalog_facility_capabilities.rs"]
mod facility_capabilities;
#[path = "catalog_facility_instances.rs"]
mod facility_instances;
#[path = "catalog_facility_medbed.rs"]
mod facility_medbed;
#[path = "catalog_facility_seats.rs"]
mod facility_seats;
#[path = "catalog_fuel_capacity.rs"]
mod fuel_capacity;
#[path = "catalog_facility_interior.rs"]
mod interior;
#[path = "catalog_inventory_interactions.rs"]
mod inventory_interactions;

#[path = "catalog_inventory_containers.rs"]
mod inventory_containers;
pub(crate) use inventory_containers::entities as inventory_entities;
#[path = "catalog_jump_drive.rs"]
mod jump_drive;
#[path = "catalog_missile_configuration.rs"]
mod missile_configuration;
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

fn catalog() -> TestResult<Value> {
    let (_directory, archive) = ammo_source::archive(ammo_source::AmmoSource::CyclicMembers)?;
    catalog_from(&archive)
}

fn catalog_from(archive: &Archive) -> TestResult<Value> {
    let options = CatalogOptions {
        publication_mode: p4kx_export::CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
    };
    let built = build_catalog(archive, &options)?;
    assert!(
        !built.summary().export_health().is_complete(),
        "the exact source errors remain visible alongside readable components"
    );
    Ok(serde_json::from_slice(built.json())?)
}

fn item(catalog: &Value, guid: u128) -> TestResult<&Value> {
    let guid = common::guid(guid);
    proof::rows(catalog, "/items")?
        .iter()
        .find(|row| row.get("guid").and_then(Value::as_str) == Some(guid.as_str()))
        .ok_or_else(|| format!("item {guid}").into())
}

fn owner<'catalog>(
    catalog: &'catalog Value,
    table: &str,
    guid: u128,
) -> TestResult<&'catalog Value> {
    let guid = common::guid(guid);
    proof::rows(catalog, table)?
        .iter()
        .find(|row| row.pointer("/ownerGuid/value").and_then(Value::as_str) == Some(guid.as_str()))
        .ok_or_else(|| format!("physical owner {guid} in {table}").into())
}

fn single(owner: &Value, field: &str, value: f32) -> TestResult {
    let finite: Value = serde_json::from_str(&serde_json::to_string(&value)?)?;
    proof::present(
        owner,
        field,
        &json!({"rawBits":format!("{:08x}", value.to_bits()), "finite":finite}),
        Some(true),
    )?;
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
    let generator = owner(&catalog, "/configurations/shieldGenerators/rows", SHIELD)?;
    for (field, value) in [
        ("maxHealth", 8000.0),
        ("maxRegen", 160.0),
        ("decayRatio", 0.25),
        ("downedRegenDelay", 5.0),
        ("damagedRegenDelay", 2.5),
    ] {
        proof::present(generator, field, &shield::number(value), Some(true))?;
    }
    let durability = owner(&catalog, "/configurations/durability/rows", SHIELD)?;
    single(durability, "health", 450.0)?;
    for path in [
        "/configurations/weaponActions/rows",
        "/configurations/missileConfigurations/rows",
        "/configurations/resourceComponents/rows",
    ] {
        assert!(owner(&catalog, path, SHIELD).is_err());
    }
    Ok(())
}

#[test]
fn a_quantum_drive_carries_its_range_and_drive_parameters() -> TestResult {
    let catalog = catalog()?;
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

fn quantum_drive(catalog: &Value, guid: u128) -> TestResult<&Value> {
    owner(catalog, "/configurations/quantumDrives/rows", guid)
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
    let catalog = catalog()?;
    let missile = owner(
        &catalog,
        "/configurations/missileConfigurations/rows",
        MISSILE,
    )?;
    let targeting = proof::at(missile, "/targeting/value")?;
    proof::present(
        targeting,
        "trackingSignalType",
        &json!("Infrared"),
        Some(true),
    )?;
    for (field, value) in [
        ("lockTime", 2.0),
        ("lockingAngle", 45.0),
        ("lockRangeMin", 100.0),
        ("lockRangeMax", 5000.0),
    ] {
        single(targeting, field, value)?;
    }
    single(proof::at(missile, "/guidance/value")?, "linearSpeed", 300.0)?;
    let ordnance = owner(&catalog, "/configurations/explosives/rows", MISSILE)?;
    for field in ["ownerRecordIndex", "componentOrdinal"] {
        assert_eq!(missile.get(field), ordnance.get(field));
    }
    assert_eq!(proof::at(ordnance, "/configuration/value/type")?, "missile");
    let explosion = proof::at(ordnance, "/configuration/value/value/explosionParams/value")?;
    single(explosion, "minRadius", 2.0)?;
    single(explosion, "maxRadius", 12.0)?;
    canonical_weapons::channels(proof::at(explosion, "/damage/value")?, 1500.0, 0.0)?;
    Ok(())
}

#[test]
fn a_health_only_item_keeps_its_original_health_without_specialist_rows() -> TestResult {
    let catalog = catalog()?;
    item(&catalog, PLAIN)?;
    let durability = owner(&catalog, "/configurations/durability/rows", PLAIN)?;
    single(durability, "health", 450.0)?;
    for path in [
        "/configurations/weaponActions/rows",
        "/configurations/shieldGenerators/rows",
        "/configurations/quantumDrives/rows",
        "/configurations/resourceComponents/rows",
        "/configurations/missileConfigurations/rows",
    ] {
        assert!(owner(&catalog, path, PLAIN).is_err());
    }
    Ok(())
}

#[path = "catalog_resource_topology.rs"]
mod resource_topology;
