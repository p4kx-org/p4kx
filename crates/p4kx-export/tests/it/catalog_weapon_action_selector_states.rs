// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, DataForgeError, Enum, Field, Value as Raw, ValueError};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use super::fixture::{self, ALPHA, BAD_LABEL_OFFSET, Case, ITEM_INDEX, MELEE, RAPID};
use super::{check_use, original_actions, owned, source};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_weapon_action_ancestry.rs"]
mod ancestry;

const LAUNCHER: &str =
    "/configurations/weaponActions/rows/0/actionNodes/0/kind/value/launchParams/value";

#[test]
fn weapon_launcher_rejects_a_readable_label_from_a_different_declared_enum() -> TestResult {
    let builder = fixture::database_for(Case::WrongDeclaration);
    original_selector(&builder, |field, declaration| {
        assert_eq!(
            declaration, "FixtureUnrelatedProjectileType",
            "the exact original enum name"
        );
        assert!(
            matches!(field.value(), Raw::Enum(label) if *label == ALPHA),
            "the different Enum still has the same readable label"
        );
        Ok(())
    })?;
    let catalog = diagnostic(&builder)?;
    let launcher = proof::at(&catalog, LAUNCHER)?;
    let selector = proof::cell(launcher, "projectileType", "unsupported")?;
    assert_eq!(
        proof::at(selector, "/sourceSetComplete")?,
        &json!(true),
        "the declaration supplies sufficient proof of the rejection"
    );
    let uses = proof::uses(&catalog, selector)?;
    assert!(
        uses.iter().any(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeEnum")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("name")
                && used.get("role").and_then(Value::as_str) == Some("selector")
        }),
        "the actual enum declaration participates in the rejection"
    );
    assert!(
        uses.iter()
            .all(|used| used.pointer("/location/field").and_then(Value::as_str) != Some("$value")),
        "the rejected declaration leaves the label unconsulted"
    );
    proof::closure(&catalog)
}

#[test]
fn weapon_launcher_marks_unreadable_label_under_the_correct_declaration() -> TestResult {
    let builder = fixture::database_for(Case::UnreadableLabel);
    let ordinal = original_selector(&builder, |field, declaration| {
        assert_eq!(
            declaration, "EProjectileType",
            "the admitted original declaration"
        );
        assert!(
            matches!(field.value(), Raw::Error(ValueError::InvalidString { offset, .. })
                if *offset == BAD_LABEL_OFFSET),
            "the actual stored Enum string word fails"
        );
        Ok(())
    })?;
    let catalog = diagnostic(&builder)?;
    let launcher = proof::at(&catalog, LAUNCHER)?;
    let selector = proof::cell(launcher, "projectileType", "unreadable")?;
    assert_eq!(
        proof::at(selector, "/sourceSetComplete")?,
        &json!(false),
        "the admitted but failed label read leaves incomplete evidence"
    );
    assert_eq!(
        proof::at(selector, "/containsEncryptedSource")?,
        &Value::Null,
        "plain but incomplete evidence is unknown"
    );
    assert!(
        proof::uses(&catalog, selector)?
            .iter()
            .all(|used| used.get("role").and_then(Value::as_str) != Some("attempted")),
        "the failed label does not contribute to the emitted value"
    );
    let indices = json!([ITEM_INDEX.to_string(), ordinal.to_string()]);
    assert!(
        proof::rows(&catalog, "/provenance/uses")?
            .iter()
            .any(|used| {
                used.get("role").and_then(Value::as_str) == Some("attempted")
                    && used.pointer("/location/namespace").and_then(Value::as_str)
                        == Some("dataforgeValue")
                    && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
                    && used.pointer("/location/indices") == Some(&indices)
            }),
        "the inventory retains the attempted label at its original occurrence"
    );
    proof::closure(&catalog)
}

#[test]
fn weapon_launcher_diagnoses_the_unreadable_original_enum_declaration() -> TestResult {
    let mut bytes = fixture::database().build();
    proof::declaration_fixture::corrupt_definition_name(&mut bytes, "EProjectileType")?;
    original_selector_in(&bytes, |field, declaration| {
        assert!(
            matches!(
                declaration.name(),
                Err(DataForgeError::InvalidString { .. })
            ),
            "the physical Enum declaration name fails"
        );
        assert!(
            matches!(field.value(), Raw::Enum(label) if *label == ALPHA),
            "the actual stored label is still readable"
        );
        Ok(())
    })?;
    let catalog = proof::declaration_fixture::diagnostic_bytes(bytes)?;
    let launcher = proof::at(&catalog, LAUNCHER)?;
    let selector = proof::cell(launcher, "projectileType", "unreadable")?;
    assert_eq!(
        proof::at(selector, "/sourceSetComplete")?,
        &json!(false),
        "the missing declaration proof is incomplete"
    );
    assert!(
        proof::rows(&catalog, "/exportHealth/diagnostics")?
            .iter()
            .any(|row| row.get("code").and_then(Value::as_str) == Some("unreadableSource")),
        "the original declaration read is diagnosed as unreadable"
    );
    proof::closure(&catalog)
}

#[test]
fn weapon_action_preserves_other_admitted_source_bodies_in_normal_publication() -> TestResult {
    let builder = fixture::database_for(Case::AuthoredBody);
    let original = original_melee_flag(&builder)?;
    let (_folder, archive) = common::open(&builder, DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let node = proof::at(
        &catalog,
        "/configurations/weaponActions/rows/0/actionNodes/3",
    )?;
    proof::present(node, "sourceType", &json!(MELEE), Some(true))?;
    assert_eq!(
        proof::at(node, "/kind/type")?,
        "authored",
        "the admitted source body is retained"
    );
    let roots = proof::at(node, "/kind/value/roots")?;
    let [field] = roots
        .as_array()
        .ok_or("ordered original action member roots")?
        .as_slice()
    else {
        return Err("one inherited original action field".into());
    };
    proof::present(field, "name", &json!("hasReloadModesOnUI"), Some(true))?;
    assert_eq!(
        proof::at(field, "/value/valueIndex")?,
        "0",
        "the authored body link is local to its own arena"
    );
    let value = proof::at(node, "/kind/value/values/0")?;
    let flag = proof::present(
        value,
        "value",
        &json!({"type":"boolean","value":false}),
        Some(true),
    )?;
    check_use(&catalog, flag, original, "payload")?;
    proof::closure(&catalog)
}

fn original_selector(
    builder: &DataForgeBuilder,
    check: impl FnOnce(&Field<'_>, &str) -> TestResult,
) -> TestResult<u64> {
    original_selector_in(&builder.build(), |field, declaration| {
        check(field, declaration.name()?)
    })
}

fn original_selector_in(
    bytes: &[u8],
    check: impl FnOnce(&Field<'_>, Enum<'_>) -> TestResult,
) -> TestResult<u64> {
    let forge = DataForge::open(bytes)?;
    let tree = forge
        .record(ITEM_INDEX)
        .ok_or("original Item owner")?
        .tree()?;
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("original component slots".into());
    };
    let actions = original_actions(&forge, components.get(1).ok_or("original physical Weapon")?)?;
    let rapid = owned(actions.first().ok_or("original first action")?, RAPID)?;
    let launcher = owned(
        rapid
            .field("launchParams")
            .ok_or("original launcher pointer")?,
        "SProjectileLauncher",
    )?;
    let selector = launcher
        .fields()
        .iter()
        .find(|field| field.name() == "projectileType")
        .ok_or("original selector field")?;
    let definition = forge
        .enums()
        .nth(usize::from(selector.storage().target()))
        .ok_or("original enum declaration")?;
    check(selector, definition)?;
    source::ordinal(&tree, selector.value())
}

fn original_melee_flag(builder: &DataForgeBuilder) -> TestResult<u64> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge
        .record(ITEM_INDEX)
        .ok_or("original Item owner")?
        .tree()?;
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("original component slots".into());
    };
    let actions = original_actions(&forge, components.get(1).ok_or("original physical Weapon")?)?;
    let melee = owned(actions.get(4).ok_or("original other action slot")?, MELEE)?;
    let flag = melee
        .field("hasReloadModesOnUI")
        .ok_or("original inherited flag")?;
    assert!(
        matches!(flag, Raw::Bool(false)),
        "the exact inherited bool survives source read"
    );
    source::ordinal(&tree, flag)
}

fn diagnostic(builder: &DataForgeBuilder) -> TestResult<Value> {
    let (_folder, archive) = common::open(builder, DcbOrigin::Plain)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}
