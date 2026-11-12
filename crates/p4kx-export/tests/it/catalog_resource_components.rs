// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_resource_component_fixture.rs"]
mod fixture;

const COMPONENTS: &str = "/configurations/resourceComponents/rows";

#[test]
fn authored_resource_states_retain_pairing_units_and_encryption() -> TestResult {
    let database = fixture::database();
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        validator
            .validate(&catalog)
            .map_err(|error| format!("catalog schema: {error}"))?;
        let row = component(&catalog, fixture::OWNER)?;
        check_component(row, encrypted)?;
        let state = proof::at(row, "/states/value/0/value")?;
        check_state(state, encrypted)?;
        let deltas = proof::rows(state, "/deltas/value")?;
        check_deltas(deltas, encrypted)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn resource_only_and_specialist_items_share_the_canonical_wire_contract() -> TestResult {
    let (_, catalog) = common::build(&fixture::database())?;
    let rows = proof::rows(&catalog, COMPONENTS)?;
    assert_eq!(
        rows.len(),
        2,
        "one physical configuration per resource component"
    );
    for guid in [fixture::OWNER, fixture::SPECIALIST] {
        let item = proof::rows(&catalog, "/items")?
            .iter()
            .find(|item| item["guid"] == common::guid(guid))
            .ok_or("resource item")?;
        assert!(
            item["stats"].is_null(),
            "resource facts use their configuration table"
        );
        let configuration = component(&catalog, guid)?;
        proof::present(
            configuration,
            "ownerGuid",
            &json!(common::guid(guid)),
            Some(false),
        )?;
    }
    Ok(())
}

fn check_component(row: &Value, encrypted: bool) -> TestResult {
    for (field, expected) in [
        ("isResourceNetworked", json!(true)),
        ("isRelay", json!(true)),
        ("isConnectedToRoom", json!(false)),
        ("wirelessConnection", json!(false)),
        ("defaultPriority", json!(0)),
    ] {
        proof::present(row, field, &expected, Some(encrypted))?;
    }
    float(row, "repairRestoreRatio", -0.0, encrypted)?;
    proof::cell(row, "controlBlocks", "present")?;
    proof::cell(row, "controlParameters", "absent")?;
    let repair = proof::at(row, "/selfRepair/value")?;
    proof::present(repair, "maxRepairCount", &json!(0), Some(encrypted))?;
    float(repair, "timeToRepair", 0.0, encrypted)?;
    float(repair, "healthRatio", 0.25, encrypted)?;
    Ok(())
}

fn check_state(state: &Value, encrypted: bool) -> TestResult {
    proof::present(state, "sourcePosition", &json!(0), Some(encrypted))?;
    proof::present(state, "name", &json!("FixtureEnabled"), Some(encrypted))?;
    let signature = proof::at(state, "/signatureParams/value/EMSignature/value")?;
    float(signature, "nominalSignature", 0.5, encrypted)?;
    float(signature, "decayRate", 0.0, encrypted)?;
    let low = proof::at(state, "/powerRanges/value/low/value")?;
    proof::present(low, "start", &json!(0), Some(encrypted))?;
    proof::present(low, "registerRange", &json!(false), Some(encrypted))?;
    float(low, "modifier", 0.0, encrypted)?;
    Ok(())
}

fn check_deltas(deltas: &[Value], encrypted: bool) -> TestResult {
    let [
        generation,
        empty,
        first,
        consumption,
        second,
        storage,
        reflection,
        future,
    ] = deltas
    else {
        return Err("all eight authored delta slots".into());
    };
    assert_eq!(empty["state"], "null", "null delta keeps its physical slot");
    check_delta_positions(deltas, encrypted)?;
    check_generation_and_consumption(generation, consumption, encrypted)?;
    check_conversion(first, 4, 1.25, true, fixture::MISSING, encrypted)?;
    check_conversion(second, 8, 3.5, false, fixture::RESOURCE, encrypted)?;
    check_storage_and_reflection(storage, reflection, encrypted)?;
    check_future(future, encrypted)
}

fn check_delta_positions(deltas: &[Value], encrypted: bool) -> TestResult {
    for (position, (slot, kind)) in deltas
        .iter()
        .zip([
            "generation",
            "",
            "conversion",
            "consumption",
            "conversion",
            "storage",
            "networkReflection",
            "unknownDelta",
        ])
        .enumerate()
    {
        if !kind.is_empty() {
            let delta = proof::at(slot, "/value")?;
            assert_eq!(delta["kind"], kind, "concrete authored delta type");
            proof::present(delta, "sourcePosition", &json!(position), Some(encrypted))?;
        }
    }
    Ok(())
}

fn check_generation_and_consumption(
    generation: &Value,
    consumption: &Value,
    encrypted: bool,
) -> TestResult {
    let unit = proof::at(
        generation,
        "/value/generation/value/resourceAmountPerSecond/value",
    )?;
    proof::present(
        unit,
        "units",
        &json!({"type":"int32", "value":17}),
        Some(encrypted),
    )?;
    let consumed = proof::at(consumption, "/value")?;
    float(consumed, "minimumConsumptionFraction", 0.0, encrypted)?;
    let unit = proof::at(consumed, "/consumption/value/resourceAmountPerSecond/value")?;
    proof::present(
        unit,
        "centiResourceUnits",
        &json!({"type":"int32","value":0}),
        Some(encrypted),
    )?;
    Ok(())
}

fn check_storage_and_reflection(
    storage: &Value,
    reflection: &Value,
    encrypted: bool,
) -> TestResult {
    let storage = proof::at(storage, "/value")?;
    proof::present(storage, "discharge", &json!(true), Some(encrypted))?;
    let output = proof::at(storage, "/generation/value/resourceAmountPerSecond/value")?;
    proof::present(
        output,
        "microResourceUnits",
        &json!({"type":"int32","value":-8}),
        Some(encrypted),
    )?;
    assert_eq!(
        proof::at(
            storage,
            "/consumption/value/resourceAmountPerSecond/value/kind"
        )?,
        "unknownUnit",
        "an unknown amount unit retains its own original type"
    );
    let reflection = proof::at(reflection, "/value")?;
    proof::present(
        reflection,
        "resource",
        &json!("FixtureNewResource"),
        Some(encrypted),
    )?;
    proof::present(
        reflection,
        "binaryEvaluation",
        &json!(false),
        Some(encrypted),
    )?;
    Ok(())
}

fn check_conversion(
    slot: &Value,
    consumed: i32,
    generated: f32,
    bounded: bool,
    resource: u128,
    encrypted: bool,
) -> TestResult {
    let conversion = proof::at(slot, "/value")?;
    let input = proof::at(conversion, "/consumption/value")?;
    proof::present(input, "resource", &json!("Power"), Some(encrypted))?;
    let input_unit = proof::at(input, "/resourceAmountPerSecond/value")?;
    proof::present(
        input_unit,
        "units",
        &json!({"type":"int32","value":consumed}),
        Some(encrypted),
    )?;
    let output = proof::at(conversion, "/generation/value")?;
    proof::present(output, "resource", &json!("Coolant"), Some(encrypted))?;
    let output_unit = proof::at(output, "/resourceAmountPerSecond/value")?;
    proof::present(
        output_unit,
        "standardResourceUnits",
        &json!({
            "type":"f32", "rawBits":format!("{:08x}", generated.to_bits()), "finite":generated,
        }),
        Some(encrypted),
    )?;
    proof::present(
        conversion,
        "noOverGeneration",
        &json!(bounded),
        Some(encrypted),
    )?;
    let composition = proof::at(
        conversion,
        "/generatedComposition/value/values/value/0/value",
    )?;
    float(composition, "ratio", 0.5, encrypted)?;
    let target = proof::at(composition, "/containerResource/value")?;
    proof::present(
        target,
        "guid",
        &json!(common::guid(resource)),
        Some(encrypted),
    )?;
    let status = if resource == fixture::MISSING {
        "dangling"
    } else {
        "present"
    };
    assert_eq!(target["status"], status, "the actual target disposition");
    Ok(())
}

fn check_future(slot: &Value, encrypted: bool) -> TestResult {
    let future = proof::at(slot, "/value")?;
    proof::present(
        future,
        "sourceType",
        &json!("FixtureFutureDelta"),
        Some(encrypted),
    )?;
    let [field] = proof::rows(future, "/additionalFields/roots")? else {
        return Err("the unknown delta's original authored member".into());
    };
    proof::present(field, "name", &json!("fixtureRule"), Some(encrypted))?;
    let index = proof::at(field, "/value/valueIndex")?
        .as_str()
        .ok_or("the original numeric field's value index")?
        .parse::<usize>()?;
    let value = proof::rows(future, "/additionalFields/values")?
        .get(index)
        .ok_or("the retained unknown numeric value")?;
    proof::present(
        value,
        "value",
        &json!({"type":"number","value":{
            "type":"f32","rawBits":format!("{:08x}", fixture::NAN_BITS),"finite":null
        }}),
        Some(encrypted),
    )?;
    Ok(())
}

fn float(owner: &Value, field: &str, number: f32, encrypted: bool) -> TestResult {
    proof::present(
        owner,
        field,
        &json!({
            "rawBits":format!("{:08x}", number.to_bits()), "finite":number,
        }),
        Some(encrypted),
    )?;
    Ok(())
}

pub(super) fn component(catalog: &Value, guid: u128) -> TestResult<&Value> {
    let owner = common::guid(guid);
    proof::rows(catalog, COMPONENTS)?
        .iter()
        .find(|row| row.pointer("/ownerGuid/value").and_then(Value::as_str) == Some(owner.as_str()))
        .ok_or_else(|| "the exact physical resource owner".into())
}
