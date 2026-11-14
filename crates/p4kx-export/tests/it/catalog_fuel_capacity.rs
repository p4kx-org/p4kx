// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::loadouts_support::{loadout, port};

#[path = "catalog_fuel_capacity_fixture.rs"]
mod fixture;
use fixture::{
    DOCKED, HYDROGEN, HYDROGEN_TANK, INTAKE, Layout, MISSING, QUANTUM, QUANTUM_TANK, SHIP,
};

const CONTAINERS: &str = "/configurations/resourceContainers/rows";
const TANKS: &str = "/configurations/fuelTanks/rows";
const INTAKES: &str = "/configurations/fuelIntakes/rows";

#[test]
fn fuel_and_capacity_keep_exact_original_units_values_and_encryption() -> TestResult {
    let database = fixture::database(Layout::Full);
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        validator
            .validate(&catalog)
            .map_err(|error| format!("catalog schema: {error}"))?;
        check_rates(&catalog, encrypted)?;
        check_units(&catalog, encrypted)?;
        check_composition(&catalog, encrypted)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn unconfigured_capacity_stays_absent_and_nonfinite_values_keep_their_bits() -> TestResult {
    let (_built, catalog) = common::build(&fixture::database(Layout::CapacityAbsent))?;
    let [ship] = exact_rows(&catalog, CONTAINERS, SHIP)?;
    proof::cell(ship, "capacity", "absent")?;
    let fill = proof::cell(ship, "defaultCompositionFillFactor", "present")?;
    assert_eq!(
        proof::at(fill, "/value/rawBits")?,
        &json!(format!("{:08x}", fixture::FUEL_NAN_BITS)),
        "the physical ship-owned field preserves its exact nonfinite token"
    );
    assert_eq!(proof::at(fill, "/value/finite")?, &Value::Null);
    Ok(())
}

#[test]
fn resolved_loadout_instances_share_the_original_configuration_without_expanding_docked_ships()
-> TestResult {
    let (_built, catalog) = common::build(&fixture::database(Layout::Full))?;
    let ship = loadout(&catalog, SHIP)?;
    let ports = proof::at(ship, "/ports")?;
    for (name, source) in [("fuel_a", "loadout"), ("fuel_b", "port_default")] {
        let mount = port(ports, name)?;
        assert_eq!(
            proof::at(mount, "/source")?,
            source,
            "the authored override or default applies"
        );
        let selected = proof::at(mount, "/sourceConstraints/attachedItem")?;
        proof::present(
            selected,
            "targetGuid",
            &json!(common::guid(HYDROGEN_TANK)),
            Some(false),
        )?;
        proof::present(selected, "targetRecordIndex", &json!(1), Some(false))?;
    }
    let intake = port(ports, "intake")?;
    proof::present(
        proof::at(intake, "/sourceConstraints/attachedItem")?,
        "targetGuid",
        &json!(common::guid(INTAKE)),
        Some(false),
    )?;
    let docked = port(ports, "hangar")?;
    assert_eq!(
        proof::at(docked, "/itemGuid")?,
        &json!(common::guid(DOCKED))
    );
    assert_eq!(proof::at(docked, "/dockedVehicle")?, &json!(true));
    assert!(
        proof::rows(docked, "/ports")?.is_empty(),
        "docked ship ports remain outside this placement"
    );
    assert_eq!(
        owned(&catalog, TANKS, HYDROGEN_TANK)?.len(),
        1,
        "the shared canonical configuration occurs once"
    );
    Ok(())
}

fn check_rates(catalog: &Value, encrypted: bool) -> TestResult {
    let [tank] = exact_rows(catalog, TANKS, HYDROGEN_TANK)?;
    check_float(tank, "hydrogenMaxFlowMultiplier", 1.5, encrypted)?;
    check_float(tank, "quantumMaxFlowMultiplier", 0.25, encrypted)?;
    let opened = proof::cell(tank, "openState", "present")?;
    assert_eq!(
        proof::at(opened, "/value/type")?,
        "weakReference",
        "the original local weak pointer shape"
    );
    proof::cell(tank, "closedState", "null")?;
    let audio = proof::at(tank, "/fuelFlowLoopStartAudioTrigger/value")?;
    proof::present(
        audio,
        "audioTrigger",
        &json!("FixtureStart"),
        Some(encrypted),
    )?;
    let rtpc = proof::at(tank, "/fuelFillDrainRateAudioRtpc/value")?;
    proof::present(rtpc, "rtpc", &json!("FixtureRate"), Some(encrypted))?;
    let [intake] = exact_rows(catalog, INTAKES, INTAKE)?;
    check_float(intake, "fuelPushRate", 1.75, encrypted)?;
    check_float(intake, "minimumRate", -0.0, encrypted)?;
    Ok(())
}

fn check_units(catalog: &Value, encrypted: bool) -> TestResult {
    let [hydrogen] = exact_rows(catalog, CONTAINERS, HYDROGEN_TANK)?;
    let centi = proof::at(hydrogen, "/capacity/value")?;
    assert_eq!(
        proof::at(centi, "/kind")?,
        "centiCargoUnit",
        "the exact Centi unit"
    );
    proof::present(centi, "centiSCU", &json!(125), Some(encrypted))?;
    let [micro, standard, future] = exact_rows(catalog, CONTAINERS, QUANTUM_TANK)?;
    let micro = proof::at(micro, "/capacity/value")?;
    assert_eq!(
        proof::at(micro, "/kind")?,
        "microCargoUnit",
        "the exact Micro unit"
    );
    proof::present(micro, "microSCU", &json!(0), Some(encrypted))?;
    let standard = proof::at(standard, "/capacity/value")?;
    check_float(standard, "standardCargoUnits", 2.25, encrypted)?;
    let future = proof::at(future, "/capacity/value")?;
    assert_eq!(
        proof::at(future, "/kind")?,
        "unknownUnit",
        "an admitted but unmapped unit stays distinct"
    );
    proof::present(
        future,
        "sourceType",
        &json!("FixtureCargoUnit"),
        Some(encrypted),
    )?;
    check_extra(
        future,
        "fixtureUnitAmount",
        &json!({"type":"f32", "rawBits":"40600000", "finite":3.5}),
        encrypted,
    )?;
    let [ship] = exact_rows(catalog, CONTAINERS, SHIP)?;
    proof::cell(ship, "capacity", "null")?;
    Ok(())
}

fn check_composition(catalog: &Value, encrypted: bool) -> TestResult {
    let [container] = exact_rows(catalog, CONTAINERS, HYDROGEN_TANK)?;
    proof::present(container, "immutable", &json!(false), Some(encrypted))?;
    proof::present(
        container,
        "generateRandomQuality",
        &json!(true),
        Some(encrypted),
    )?;
    proof::present(
        container,
        "mutabilityLevel",
        &json!("FixtureMutable"),
        Some(encrypted),
    )?;
    check_float(container, "defaultCompositionFillFactor", 0.75, encrypted)?;
    let rows = proof::rows(container, "/defaultComposition/value")?;
    for (row, (guid, weight, status)) in rows.iter().zip([
        (HYDROGEN, 0.25, "present"),
        (MISSING, 0.5, "dangling"),
        (QUANTUM, 0.25, "present"),
    ]) {
        let value = proof::at(row, "/value")?;
        let reference = proof::at(value, "/entry/value")?;
        proof::present(
            reference,
            "guid",
            &json!(common::guid(guid)),
            Some(encrypted),
        )?;
        assert_eq!(
            proof::at(reference, "/status")?,
            status,
            "the original target disposition"
        );
        check_float(value, "weight", weight, encrypted)?;
        check_extra(
            value,
            "fixtureQuality",
            &json!({"type":"uint32", "value":12}),
            encrypted,
        )?;
    }
    assert_eq!(rows.len(), 3, "all physical composition slots survive");
    let excluded = proof::at(container, "/exclusiveResources/value/0/value")?;
    proof::present(
        excluded,
        "guid",
        &json!(common::guid(MISSING)),
        Some(encrypted),
    )?;
    assert_eq!(
        proof::at(excluded, "/status")?,
        "dangling",
        "a dangling resource restriction remains visible"
    );
    Ok(())
}

fn check_extra(owner: &Value, field: &str, number: &Value, encrypted: bool) -> TestResult {
    let [extra] = proof::rows(owner, "/additionalFields/roots")? else {
        return Err("one original additional member".into());
    };
    proof::present(extra, "name", &json!(field), Some(encrypted))?;
    let value_index = proof::at(extra, "/value/valueIndex")?
        .as_str()
        .ok_or("additional member arena index")?
        .parse::<usize>()?;
    let original = proof::rows(owner, "/additionalFields/values")?
        .get(value_index)
        .ok_or("original additional value")?;
    proof::present(
        original,
        "value",
        &json!({"type":"number", "value":number}),
        Some(encrypted),
    )?;
    Ok(())
}

fn check_float(owner: &Value, field: &str, value: f32, encrypted: bool) -> TestResult {
    proof::present(
        owner,
        field,
        &json!({"finite": value, "rawBits": format!("{:08x}", value.to_bits())}),
        Some(encrypted),
    )?;
    Ok(())
}

fn owned<'catalog>(
    catalog: &'catalog Value,
    path: &str,
    guid: u128,
) -> TestResult<Vec<&'catalog Value>> {
    let expected = common::guid(guid);
    Ok(proof::rows(catalog, path)?
        .iter()
        .filter(|row| {
            row.pointer("/ownerGuid/value").and_then(Value::as_str) == Some(expected.as_str())
        })
        .collect())
}

fn exact_rows<'catalog, const N: usize>(
    catalog: &'catalog Value,
    path: &str,
    guid: u128,
) -> TestResult<[&'catalog Value; N]> {
    owned(catalog, path, guid)?.try_into().map_err(|_| {
        format!("expected {N} physical configuration rows for the fixture owner").into()
    })
}
