// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::{DataForge, Value as Raw};
use serde_json::{Value, json};

use super::{check_single, original_actions, owned, source};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_weapon_action_wrapper_fixture.rs"]
mod fixture;

use fixture::{
    AIM, BEAM, BURST, CHARGED, CONDITIONAL, DELAY_UNIT, DYNAMIC, HELD, MODE, OWNER, PARALLEL,
    RAPID, SEQUENCE, SINGLE, TOGGLE,
};

struct OriginalEdges {
    sequence_child: u64,
    default_child: u64,
    aim: u64,
}

#[test]
fn weapon_action_wrappers_preserve_original_edges_and_entry_metadata() -> TestResult {
    let database = fixture::database();
    let edges = original_edges(&database.build())?;
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    check_legacy(&catalog)?;
    let row = proof::at(&catalog, "/configurations/weaponActions/rows/0")?;
    proof::present(row, "ownerRecordIndex", &json!(OWNER), Some(true))?;
    proof::present(row, "componentOrdinal", &json!(1), Some(true))?;
    let [sequence, parallel, conditional, dynamic, charged] =
        proof::rows(row, "/fireActions/value")?
    else {
        return Err("five original fire-action root slots".into());
    };
    for (slot, index) in [
        (sequence, 0),
        (parallel, 1),
        (conditional, 2),
        (dynamic, 3),
        (charged, 4),
    ] {
        assert_eq!(
            proof::at(slot, "/value")?,
            &json!(index),
            "the original ordered root slot"
        );
    }
    proof::present(row, "aimAction", &json!(5), Some(true))?;
    check_nodes(&catalog, row, &edges)?;
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

fn check_nodes(catalog: &Value, row: &Value, original: &OriginalEdges) -> TestResult {
    let [
        sequence,
        parallel,
        conditional,
        dynamic,
        charged,
        aim,
        sequence_child,
        toggle,
        choice,
        default,
        dynamic_child,
        charged_child,
        toggle_child,
    ] = proof::rows(row, "/actionNodes")?
    else {
        return Err("original roots and every readable ordered child".into());
    };
    for (node, source_type) in [
        (sequence, SEQUENCE),
        (parallel, PARALLEL),
        (conditional, CONDITIONAL),
        (dynamic, DYNAMIC),
        (charged, CHARGED),
        (aim, AIM),
        (sequence_child, RAPID),
        (toggle, TOGGLE),
        (choice, BURST),
        (default, BEAM),
        (dynamic_child, SINGLE),
        (charged_child, RAPID),
        (toggle_child, BEAM),
    ] {
        proof::present(node, "sourceType", &json!(source_type), Some(true))?;
    }
    for (node, ordinal) in [
        (sequence_child, original.sequence_child),
        (default, original.default_child),
        (aim, original.aim),
    ] {
        let cell = proof::present(
            node,
            "sourceValueOrdinal",
            &json!(ordinal.to_string()),
            Some(true),
        )?;
        check_occurrence(catalog, cell, ordinal)?;
    }
    check_sequence(sequence)?;
    check_parallel(parallel)?;
    proof::present(
        body(toggle, "toggle")?,
        "weaponAction",
        &json!(12),
        Some(true),
    )?;
    check_condition(conditional, dynamic)?;
    check_charge(charged)
}

fn check_parallel(parallel: &Value) -> TestResult {
    let parallel = body(parallel, "parallel")?;
    let [child, null] = proof::rows(parallel, "/weaponActions/value")? else {
        return Err("two original parallel slots".into());
    };
    assert_eq!(
        proof::at(child, "/value")?,
        &json!(7),
        "parallel child local index"
    );
    assert_eq!(proof::at(null, "/state")?, "null", "parallel null slot");
    Ok(())
}

fn check_sequence(node: &Value) -> TestResult {
    let sequence = body(node, "sequence")?;
    proof::present(sequence, "mode", &json!(MODE), Some(true))?;
    let [first, second] = proof::rows(sequence, "/sequenceEntries/value")? else {
        return Err("two original sequence entries".into());
    };
    let first = proof::at(first, "/value")?;
    let second = proof::at(second, "/value")?;
    check_single(first, "/delay/value/rawBits", "3f000000")?;
    check_single(second, "/delay/value/rawBits", "80000000")?;
    proof::present(first, "unit", &json!(DELAY_UNIT), Some(true))?;
    proof::present(first, "repetitions", &json!(3), Some(true))?;
    proof::present(second, "repetitions", &json!(0), Some(true))?;
    proof::present(first, "weaponAction", &json!(6), Some(true))?;
    proof::cell(second, "weaponAction", "null")?;
    let [member] = proof::rows(sequence, "/authored/roots")? else {
        return Err("only unconsumed wrapper member remains in the original arena".into());
    };
    proof::present(member, "name", &json!("hasReloadModesOnUI"), Some(true))?;
    Ok(())
}

fn check_condition(conditional: &Value, dynamic: &Value) -> TestResult {
    let conditional = body(conditional, "conditional")?;
    let [first, second] = proof::rows(conditional, "/conditionalWeaponActions/value")? else {
        return Err("ordered original conditional choices".into());
    };
    let first = proof::at(first, "/value")?;
    let second = proof::at(second, "/value")?;
    assert_eq!(
        proof::at(
            first,
            "/condition/value/values/0/value/value/sourceType/value"
        )?,
        HELD,
        "the actual unevaluated condition type"
    );
    proof::present(first, "weaponAction", &json!(8), Some(true))?;
    proof::cell(second, "condition", "null")?;
    proof::cell(second, "weaponAction", "null")?;
    proof::present(conditional, "defaultWeaponAction", &json!(9), Some(true))?;
    let dynamic = body(dynamic, "dynamicConditional")?;
    proof::present(dynamic, "conditionalWeaponActions", &json!([]), Some(true))?;
    proof::present(dynamic, "defaultWeaponAction", &json!(10), Some(true))?;
    Ok(())
}

fn check_charge(node: &Value) -> TestResult {
    let charged = body(node, "charged")?;
    proof::present(charged, "weaponAction", &json!(11), Some(true))?;
    check_single(charged, "/overchargeTime/value/rawBits", "3e800000")?;
    check_single(charged, "/overchargedTime/value/rawBits", "3f000000")?;
    proof::present(charged, "chargeAutomatically", &json!(false), Some(true))?;
    proof::present(charged, "fireOnlyOnFullCharge", &json!(true), Some(true))?;
    let modifier = proof::at(charged, "/maxChargeModifier/value")?;
    assert_eq!(
        proof::at(modifier, "/roots/0/name/value")?,
        "damageMultiplier",
        "the charge modifier body retains its exact source field"
    );
    Ok(())
}

fn body<'node>(node: &'node Value, kind: &str) -> TestResult<&'node Value> {
    assert_eq!(proof::at(node, "/kind/type")?, kind, "typed wrapper kind");
    proof::at(node, "/kind/value")
}

fn check_occurrence(catalog: &Value, cell: &Value, ordinal: u64) -> TestResult {
    let indices = json!([OWNER.to_string(), ordinal.to_string()]);
    assert!(
        proof::uses(catalog, cell)?.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("conversion")
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && used.pointer("/location/indices") == Some(&indices)
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
        }),
        "the actual original child route is cited in its owning Record"
    );
    Ok(())
}

fn check_legacy(catalog: &Value) -> TestResult {
    let actual = proof::rows(catalog, "/items/0/stats/weapon/fireActions")?;
    let expected = [RAPID, TOGGLE, CONDITIONAL, DYNAMIC, CHARGED];
    assert_eq!(
        actual.len(),
        expected.len(),
        "the previous flattened list length"
    );
    for (row, expected) in actual.iter().zip(expected) {
        assert_eq!(
            proof::at(row, "/kind")?,
            expected,
            "the previous flattened action type"
        );
    }
    Ok(())
}

fn original_edges(bytes: &[u8]) -> TestResult<OriginalEdges> {
    let forge = DataForge::open(bytes)?;
    let tree = forge
        .record(OWNER)
        .ok_or("original weapon Record")?
        .tree()?;
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("original item components".into());
    };
    let owner = components.get(1).ok_or("original physical weapon")?;
    let actions = original_actions(&forge, owner)?;
    let sequence = owned(actions.first().ok_or("original sequence")?, SEQUENCE)?;
    let Some(Raw::Array(entries)) = sequence.field("sequenceEntries") else {
        return Err("original sequence entry slots".into());
    };
    let entry = owned(
        entries.first().ok_or("original first entry")?,
        "SWeaponSequenceEntryParams",
    )?;
    assert!(
        matches!(entry.field("repetitions"), Some(Raw::Int32(3))),
        "original repetition word"
    );
    let child = entry
        .field("weaponAction")
        .ok_or("original sequence child")?;
    let choice = owned(actions.get(2).ok_or("original conditional")?, CONDITIONAL)?;
    let default = choice
        .field("defaultWeaponAction")
        .ok_or("original conditional default")?;
    let aim = owned(owner, "SCItemWeaponComponentParams")?
        .field("aimAction")
        .ok_or("original aim action")?;
    Ok(OriginalEdges {
        sequence_child: source::ordinal(&tree, child)?,
        default_child: source::ordinal(&tree, default)?,
        aim: source::ordinal(&tree, aim)?,
    })
}
