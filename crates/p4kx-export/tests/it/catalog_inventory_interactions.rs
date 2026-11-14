// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_inventory_interactions_fixture.rs"]
mod fixture;

#[test]
fn interaction_components_preserve_slots_states_and_original_weak_links() -> TestResult {
    let (_directory, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    original_default_state(&archive)?;
    let catalog: Value =
        serde_json::from_slice(p4kx_export::build_catalog(&archive, &common::options())?.json())?;
    let rows = proof::rows(&catalog, "/configurations/interactions/rows")?;
    assert_eq!(rows.len(), 2, "one row per physical interaction component");
    check_interactable(&rows[0])?;
    check_state_machine(&rows[1])?;
    jsonschema::validator_for(&p4kx_export::catalog_schema())?
        .validate(&catalog)
        .map_err(|error| format!("schema: {error}"))?;
    proof::closure(&catalog)
}

fn original_default_state(archive: &p4kx_extraction::Archive) -> TestResult {
    use p4kx_extraction::{DataForgeEntry, Value as Original};
    let dcb = DataForgeEntry::read(archive)?;
    let forge = dcb.open()?;
    let tree = forge
        .record(0)
        .ok_or("fixture interaction record")?
        .tree()?;
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original fixture components".into());
    };
    let Some(Original::Struct(machine)) = components.get(2) else {
        return Err("original fixture state machine".into());
    };
    let Some(Original::Array(categories)) = machine.field("StateTypes") else {
        return Err("original fixture state categories".into());
    };
    let Some(Original::Struct(category)) = categories.first() else {
        return Err("original fixture state category".into());
    };
    let original = category.field("DefaultState");
    let Some(Original::Reference(link)) = original else {
        return Err(format!("original fixture default state: {original:?}").into());
    };
    assert!(
        link.location().is_some(),
        "the original fixture links to an owned state"
    );
    Ok(())
}

fn check_interactable(row: &Value) -> TestResult {
    assert_eq!(
        row["kind"], "interactable",
        "concrete component source family"
    );
    owner(row, 1)?;
    let data = proof::at(row, "/interactable/value")?;
    let slots = proof::rows(data, "/sharedInteractions/value")?;
    let [first, empty, third] = slots else {
        return Err("expected three original shared interaction slots".into());
    };
    assert_eq!(empty["state"], "null", "null remains at its original slot");
    for slot in [first, third] {
        let source = proof::at(slot, "/value")?;
        proof::present(
            source,
            "sourceType",
            &json!("FixtureSharedInteraction"),
            Some(true),
        )?;
        proof::present(source, "name", &json!("FixtureShared"), Some(true))?;
        proof::present(source, "available", &json!(false), Some(true))?;
        let raw = proof::at(source, "/additionalFields/roots/0")?;
        proof::present(raw, "name", &json!("fixturePriority"), Some(true))?;
        proof::cell(source, "displayName", "present")?;
        check_condition_list(proof::at(source, "/conditionList/value")?)?;
    }
    let point = proof::at(
        data,
        "/interactionPoints/value/0/value/interactions/value/0/value",
    )?;
    proof::present(point, "isPrimary", &json!(false), Some(true))?;
    check_owned_link(proof::at(point, "/interaction/value")?)?;
    check_tasks(data)
}

fn check_condition_list(list: &Value) -> TestResult {
    let preset = proof::at(list, "/presetList/value/0/value")?;
    proof::present(
        preset,
        "guid",
        &json!(common::guid(fixture::PRESET)),
        Some(true),
    )?;
    assert_eq!(
        preset["status"], "present",
        "condition preset retains the actual target"
    );
    let missing = proof::at(list, "/presetList/value/1/value")?;
    proof::present(
        missing,
        "guid",
        &json!(common::guid(fixture::MISSING_PRESET)),
        Some(true),
    )?;
    assert_eq!(
        missing["status"], "dangling",
        "missing preset remains distinct"
    );
    let predicate = proof::at(list, "/conditionParams/value/0/value")?;
    proof::present(
        predicate,
        "sourceType",
        &json!("FixtureCondition"),
        Some(true),
    )?;
    let flag = proof::at(predicate, "/additionalFields/values/0")?;
    proof::present(
        flag,
        "value",
        &json!({"type":"boolean","value":false}),
        Some(true),
    )?;
    assert!(
        proof::rows(list, "/conditionToHideParams/value")?.is_empty(),
        "explicit empty hiding conditions"
    );
    Ok(())
}

fn check_tasks(data: &Value) -> TestResult {
    let task = proof::at(data, "/interactionTasks/value/0/value")?;
    proof::present(task, "interaction", &json!("FixtureShared"), Some(true))?;
    proof::present(task, "triggerType", &json!("FixtureTrigger"), Some(true))?;
    let parameter = proof::at(task, "/additionalFields/roots/0")?;
    proof::present(
        parameter,
        "name",
        &json!("fixtureActionParameter"),
        Some(true),
    )?;
    assert!(
        proof::rows(data, "/actionHandlerTasks/value")?.is_empty(),
        "empty actor task array"
    );
    assert!(
        proof::rows(data, "/itemActionTasks/value")?.is_empty(),
        "empty item task array"
    );
    proof::cell(data, "menuTitle", "null")?;
    Ok(())
}

fn check_state_machine(row: &Value) -> TestResult {
    assert_eq!(
        row["kind"], "stateMachine",
        "concrete machine source family"
    );
    owner(row, 2)?;
    let types = proof::rows(row, "/stateTypes/value")?;
    let [initial, empty] = types else {
        return Err("expected two original state types".into());
    };
    let initial = proof::at(initial, "/value")?;
    proof::present(
        initial,
        "stateTypeName",
        &json!("FixturePrimary"),
        Some(true),
    )?;
    proof::present(
        initial,
        "bindingsMethod",
        &json!("FixtureBindings"),
        Some(true),
    )?;
    proof::present(
        initial,
        "networkAuthority",
        &json!("FixtureAuthority"),
        Some(true),
    )?;
    proof::present(initial, "asopReset", &json!(false), Some(true))?;
    proof::cell(initial, "defaultState", "present")?;
    let default = proof::at(initial, "/defaultState/value")?;
    check_owned_link(default)?;
    let states = proof::rows(initial, "/states/value")?;
    let [state, null] = states else {
        return Err("expected two original state pointer slots".into());
    };
    assert_eq!(null["state"], "null", "state list preserves null slot");
    check_modifiers(initial)?;
    let state = proof::at(state, "/value")?;
    check_transition(state, default)?;
    check_state_edges(state, default)?;
    let empty = proof::at(empty, "/value")?;
    proof::cell(empty, "defaultState", "null")?;
    assert!(
        proof::rows(empty, "/states/value")?.is_empty(),
        "explicit empty state list"
    );
    Ok(())
}

fn check_modifiers(initial: &Value) -> TestResult {
    let modifiers = proof::rows(initial, "/stateModifiers/value")?;
    let [cargo, other] = modifiers else {
        return Err("expected two authored concrete state modifiers".into());
    };
    let cargo = proof::at(cargo, "/value")?;
    assert_eq!(
        cargo["kind"], "cargoOccupancy",
        "exact cargo modifier family"
    );
    check_owned_link(proof::at(cargo, "/cargoVacant/value")?)?;
    proof::cell(cargo, "cargoStored", "null")?;
    let other = proof::at(other, "/value")?;
    assert_eq!(
        other["kind"], "original",
        "unknown concrete modifier retains original parameters"
    );
    let parameter = proof::at(other, "/additionalFields/roots/0")?;
    proof::present(
        parameter,
        "name",
        &json!("fixtureModifierParameter"),
        Some(true),
    )?;
    Ok(())
}

fn check_state_edges(state: &Value, default: &Value) -> TestResult {
    let points = proof::rows(state, "/interactionPoints/value")?;
    let [first_slot, null, third_slot] = points else {
        return Err("expected three original WeakPointer slots".into());
    };
    assert_eq!(
        null["state"], "null",
        "null WeakPointer slot remains in place"
    );
    let first = proof::at(first_slot, "/value")?;
    let third = proof::at(third_slot, "/value")?;
    check_owned_link(first)?;
    check_owned_link(third)?;
    proof::present(
        &first["target"],
        "sourceType",
        &json!("SInteractionPointParams"),
        Some(true),
    )?;
    assert_eq!(
        first.pointer("/target/location/value"),
        third.pointer("/target/location/value"),
        "repeated slots retain the same physical target"
    );
    assert_ne!(
        proof::at(first_slot, "/sourceUsesRefs")?,
        proof::at(third_slot, "/sourceUsesRefs")?,
        "each referring slot keeps its own physical use"
    );
    let auto = proof::at(state, "/stateAutoChange/value")?;
    proof::present(
        auto,
        "delay",
        &json!({"rawBits":format!("{:08x}",fixture::DELAY_BITS),"finite":null}),
        Some(true),
    )?;
    proof::present(auto, "runWhenStreamedOut", &json!(false), Some(true))?;
    assert_eq!(
        auto.pointer("/nextState/value/target/location/value"),
        default.pointer("/target/location/value"),
        "automatic transition retains the original target instance"
    );
    Ok(())
}

fn check_transition(state: &Value, default: &Value) -> TestResult {
    proof::present(state, "stateName", &json!("FixtureState"), Some(true))?;
    let transition = proof::at(state, "/interactions/value/0/value")?;
    check_owned_link(proof::at(transition, "/interaction/value")?)?;
    let next = proof::at(transition, "/nextState/value")?;
    check_owned_link(next)?;
    assert_eq!(
        next.pointer("/target/location/value"),
        default.pointer("/target/location/value"),
        "both authored weak slots target the same original state"
    );
    Ok(())
}

fn check_owned_link(link: &Value) -> TestResult {
    assert_eq!(
        proof::at(link, "/target/type")?,
        "instance",
        "the actual pointer target is an owned instance"
    );
    proof::present(
        link,
        "recordGuid",
        &json!(common::guid(fixture::OWNER)),
        Some(true),
    )?;
    proof::cell(&link["target"], "location", "present")?;
    Ok(())
}

fn owner(row: &Value, slot: u32) -> TestResult {
    proof::present(
        row,
        "ownerGuid",
        &json!(common::guid(fixture::OWNER)),
        Some(true),
    )?;
    proof::present(row, "ownerRecordIndex", &json!(0), Some(true))?;
    proof::present(row, "componentOrdinal", &json!(slot), Some(true))?;
    Ok(())
}
