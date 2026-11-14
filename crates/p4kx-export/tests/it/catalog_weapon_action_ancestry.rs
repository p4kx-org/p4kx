// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::{DataForge, DataType, Struct, Value as Raw};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use super::super::fixture::{
    self, ACTION, ALPHA, Case, ITEM_INDEX, LAUNCHER_BASE, LAUNCHER_MARKER, MALFORMED_HEAT, MELEE,
    NEW_ACTION, NEW_LAUNCHER, PROJECTILE, RAPID, UNRELATED,
};
use super::super::{check_use, original_actions, owned, source};
use super::diagnostic;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const FIRST: &str = "/configurations/weaponActions/rows/0";
const FIRST_LAUNCHER: &str = "/actionNodes/0/kind/value/launchParams";

#[test]
fn a_new_physical_weapon_action_descendant_keeps_its_inherited_body() -> TestResult {
    let builder = fixture::database_for(Case::NewAction);
    let chain = assert_original_chain(&builder, NEW_ACTION, &[MELEE, ACTION])?;
    let source_ordinal = inherited_flag(&builder)?;
    let (_folder, archive) = common::open(&builder, DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = proof::at(&catalog, FIRST)?;
    let node = proof::at(row, "/actionNodes/3")?;
    proof::present(node, "sourceType", &json!(NEW_ACTION), Some(true))?;
    assert_eq!(
        proof::at(node, "/kind/type")?,
        "authored",
        "a new physical descendant gets the authored representation"
    );
    let field = proof::at(node, "/kind/value/roots/0")?;
    proof::present(field, "name", &json!("hasReloadModesOnUI"), Some(true))?;
    let value = proof::at(node, "/kind/value/values/0")?;
    let flag = proof::present(
        value,
        "value",
        &json!({"type":"boolean","value":false}),
        Some(true),
    )?;
    check_use(&catalog, flag, source_ordinal, "payload")?;
    let slot = proof::at(row, "/fireActions/value/4")?;
    assert_eq!(
        proof::at(slot, "/value")?,
        &json!(3),
        "original action slot"
    );
    check_parent_evidence(&catalog, slot, &chain)?;
    proof::closure(&catalog)
}

#[test]
fn a_known_weapon_action_spelling_does_not_admit_an_unrelated_physical_parent() -> TestResult {
    let builder = fixture::database_for(Case::WrongActionParent);
    let chain = assert_original_chain(&builder, RAPID, &[UNRELATED])?;
    let catalog = diagnostic(&builder)?;
    let row = proof::at(&catalog, FIRST)?;
    let slot = proof::at(row, "/fireActions/value/0")?;
    assert_eq!(
        proof::at(slot, "/state")?,
        "unsupported",
        "the physical slot rejects its incompatible action"
    );
    assert_eq!(
        proof::rows(row, "/actionNodes")?.len(),
        2,
        "incompatible source is outside the typed action table"
    );
    check_parent_evidence(&catalog, slot, &chain)?;
    proof::closure(&catalog)
}

#[test]
fn a_new_physical_weapon_launcher_descendant_keeps_its_inherited_body() -> TestResult {
    let builder = fixture::database_for(Case::NewLauncher);
    let (chain, ordinal) = original_launcher(&builder, NEW_LAUNCHER, &[PROJECTILE, LAUNCHER_BASE])?;
    let (_built, catalog) = common::build(&builder)?;
    let slot = proof::at(proof::at(&catalog, FIRST)?, FIRST_LAUNCHER)?;
    let launcher = proof::at(slot, "/value")?;
    assert_eq!(
        proof::at(launcher, "/type")?,
        "authored",
        "a genuine unlisted launcher gets its complete authored body"
    );
    proof::present(launcher, "sourceType", &json!(NEW_LAUNCHER), Some(false))?;
    let field = proof::at(launcher, "/original/roots/0")?;
    proof::present(field, "name", &json!("projectileType"), Some(false))?;
    let value = proof::at(launcher, "/original/values/0")?;
    let label = proof::present(
        value,
        "value",
        &json!({"type":"enum","value":ALPHA}),
        Some(false),
    )?;
    check_use(&catalog, label, ordinal, "payload")?;
    check_parent_evidence(&catalog, slot, &chain)?;
    proof::closure(&catalog)
}

#[test]
fn a_known_weapon_launcher_spelling_does_not_admit_an_unrelated_physical_parent() -> TestResult {
    let builder = fixture::database_for(Case::WrongLauncherParent);
    let (chain, _) = original_launcher(&builder, PROJECTILE, &[UNRELATED])?;
    let catalog = diagnostic(&builder)?;
    let slot = proof::at(proof::at(&catalog, FIRST)?, FIRST_LAUNCHER)?;
    assert_eq!(
        proof::at(slot, "/state")?,
        "unsupported",
        "the known spelling cannot override incompatible physical ancestry"
    );
    check_parent_evidence(&catalog, slot, &chain)?;
    proof::closure(&catalog)
}

#[test]
fn a_typed_weapon_leaf_retains_other_authored_fields() -> TestResult {
    let builder = fixture::database();
    let flag = original_rapid_member(&builder, &["hasReloadModesOnUI"], |value| {
        matches!(value, Raw::Bool(false))
    })?;
    let marker = original_rapid_member(
        &builder,
        &["launchParams", "fixtureMarker"],
        |value| matches!(value, Raw::UInt32(word) if *word == LAUNCHER_MARKER),
    )?;
    let (_built, catalog) = common::build(&builder)?;
    let row = proof::at(&catalog, FIRST)?;
    let action = proof::at(row, "/actionNodes/0/kind/value/authored")?;
    check_residual(
        &catalog,
        action,
        "hasReloadModesOnUI",
        &json!({"type":"boolean","value":false}),
        flag,
    )?;
    let launcher = proof::at(proof::at(row, FIRST_LAUNCHER)?, "/value/authored")?;
    check_residual(
        &catalog,
        launcher,
        "fixtureMarker",
        &json!({
            "type":"number",
            "value":{"type":"uint32","value":LAUNCHER_MARKER}
        }),
        marker,
    )?;
    proof::closure(&catalog)
}

#[test]
fn a_typed_weapon_leaf_retains_raw_invalid_typed_source() -> TestResult {
    let builder = fixture::database_for(Case::MalformedHeat);
    let heat = original_rapid_member(
        &builder,
        &["heatPerShot"],
        |value| matches!(value, Raw::UInt32(word) if *word == MALFORMED_HEAT),
    )?;
    let catalog = diagnostic(&builder)?;
    let action = proof::at(proof::at(&catalog, FIRST)?, "/actionNodes/0/kind/value")?;
    proof::cell(action, "heatPerShot", "invalid")?;
    check_residual(
        &catalog,
        proof::at(action, "/authored")?,
        "heatPerShot",
        &json!({
            "type":"number",
            "value":{"type":"uint32","value":MALFORMED_HEAT}
        }),
        heat,
    )?;
    proof::closure(&catalog)
}

fn original_rapid_member(
    builder: &DataForgeBuilder,
    path: &[&str],
    expected: impl FnOnce(&Raw<'_>) -> bool,
) -> TestResult<u64> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(ITEM_INDEX).ok_or("original item")?.tree()?;
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("original weapon component array".into());
    };
    let actions = original_actions(&forge, components.get(1).ok_or("weapon slot")?)?;
    let mut value = actions.first().ok_or("rapid slot")?;
    for name in path {
        let Raw::Struct(owner) = value else {
            return Err("original rapid field owner".into());
        };
        value = owner.field(name).ok_or("original authored member")?;
    }
    assert!(expected(value), "exact original stored member");
    source::ordinal(&tree, value)
}

fn check_residual(
    catalog: &Value,
    arena: &Value,
    name: &str,
    expected: &Value,
    ordinal: u64,
) -> TestResult {
    let field = proof::rows(arena, "/roots")?
        .iter()
        .find(|field| field.pointer("/name/value").and_then(Value::as_str) == Some(name))
        .ok_or("retained original member")?;
    let index = proof::at(field, "/value/valueIndex")?
        .as_str()
        .ok_or("exact local original value index")?;
    let value = proof::at(arena, &format!("/values/{index}"))?;
    let cell = proof::present(value, "value", expected, Some(false))?;
    check_use(catalog, cell, ordinal, "payload")
}

fn original_launcher(
    builder: &DataForgeBuilder,
    concrete: &str,
    expected_parents: &[&str],
) -> TestResult<(Vec<u32>, u64)> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(ITEM_INDEX).ok_or("original item")?.tree()?;
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("original weapon component array".into());
    };
    let actions = original_actions(&forge, components.get(1).ok_or("weapon slot")?)?;
    let rapid = owned(actions.first().ok_or("rapid slot")?, RAPID)?;
    let field = rapid
        .fields()
        .iter()
        .find(|field| field.name() == "launchParams")
        .ok_or("stored launcher declaration")?;
    let storage = field.storage();
    assert_eq!(
        storage.data_type(),
        DataType::StrongPointer,
        "launcher storage"
    );
    assert!(!storage.is_array(), "launcher is a scalar pointer");
    let target = forge
        .struct_at(u32::from(storage.target()))
        .ok_or("launcher base declaration")?;
    assert_eq!(
        target.name()?,
        LAUNCHER_BASE,
        "the actual stored launcher base"
    );
    let launcher = owned(field.value(), concrete)?;
    let current = forge
        .struct_at(launcher.struct_index())
        .ok_or("physical launcher definition")?;
    let chain = check_chain(current, target, expected_parents)?;
    let label = launcher
        .field("projectileType")
        .ok_or("authored selector")?;
    assert!(
        matches!(label, Raw::Enum(value) if *value == ALPHA),
        "exact original label"
    );
    Ok((chain, source::ordinal(&tree, label)?))
}

fn assert_original_chain(
    builder: &DataForgeBuilder,
    name: &str,
    expected_parents: &[&str],
) -> TestResult<Vec<u32>> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(ITEM_INDEX).ok_or("original item")?.tree()?;
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("original weapon component array".into());
    };
    let weapon = owned(components.get(1).ok_or("weapon slot")?, fixture::WEAPON)?;
    let action_field = weapon
        .fields()
        .iter()
        .find(|field| field.name() == "fireActions")
        .ok_or("original fireActions declaration")?;
    let storage = action_field.storage();
    assert_eq!(
        storage.data_type(),
        DataType::StrongPointer,
        "stored pointer type"
    );
    assert!(storage.is_array(), "physical fireActions is an array");
    let target = forge
        .structs()
        .nth(usize::from(storage.target()))
        .ok_or("declared action base")?;
    assert_eq!(
        target.name()?,
        ACTION,
        "field declares the actual action base"
    );
    let current = forge
        .structs()
        .find(|row| row.name().is_ok_and(|found| found == name))
        .ok_or("original concrete declaration")?;
    check_chain(current, target, expected_parents)
}

fn check_chain(
    mut current: Struct<'_>,
    target: Struct<'_>,
    expected_parents: &[&str],
) -> TestResult<Vec<u32>> {
    let mut inspected = Vec::new();
    for expected in expected_parents {
        inspected.push(current.index());
        current = current.parent().ok_or("original parent link")?;
        assert_eq!(current.name()?, *expected, "original physical ancestry");
    }
    if current.index() != target.index() {
        assert!(
            current.parent().is_none(),
            "unrelated hierarchy ends at its own base"
        );
        inspected.push(current.index());
    }
    Ok(inspected)
}

fn inherited_flag(builder: &DataForgeBuilder) -> TestResult<u64> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(ITEM_INDEX).ok_or("original item")?.tree()?;
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("original weapon component array".into());
    };
    let actions = original_actions(&forge, components.get(1).ok_or("weapon slot")?)?;
    let extension = owned(actions.get(4).ok_or("extension slot")?, NEW_ACTION)?;
    let flag = extension
        .field("hasReloadModesOnUI")
        .ok_or("inherited flag")?;
    assert!(matches!(flag, Raw::Bool(false)), "original authored false");
    source::ordinal(&tree, flag)
}

fn check_parent_evidence(catalog: &Value, slot: &Value, chain: &[u32]) -> TestResult {
    let uses = proof::uses(catalog, slot)?;
    for index in chain {
        let indices = json!([index.to_string()]);
        assert!(
            uses.iter().any(|entry| {
                entry.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeStructDeclaration")
                    && entry.pointer("/location/indices") == Some(&indices)
                    && entry.pointer("/location/field").and_then(Value::as_str) == Some("parent")
                    && entry.get("role").and_then(Value::as_str) == Some("selector")
            }),
            "slot cites the exact inspected original parent word {index}"
        );
    }
    Ok(())
}
