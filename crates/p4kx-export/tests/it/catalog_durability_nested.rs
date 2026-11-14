// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, Value as Original};
use p4kx_testkit::{DataForgeBuilder, TestType};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_durability_nested_fixture.rs"]
mod fixture;

#[test]
fn durability_nested_initial_damage_keeps_concrete_branches_and_raw_words() -> TestResult {
    let catalog = catalog(&fixture::initial_database())?;
    let rows = proof::rows(&catalog, "/configurations/durability/rows")?;
    let [fixed, fraction, base] = rows else {
        return Err("three original Health occurrences".into());
    };
    let fixed = proof::at(fixed, "/initialDamage/value")?;
    proof::present(fixed, "randomSeed", &json!(-3), Some(true))?;
    proof::present(fixed, "minHitCount", &json!(0), Some(true))?;
    assert_eq!(
        proof::at(fixed, "/damage/value/type")?,
        "fixed",
        "actual owned damage specifier"
    );
    let raw = proof::at(fixed, "/damage/value/value/damage/value")?;
    assert_eq!(
        proof::at(raw, "/type")?,
        "info",
        "actual authored damage storage"
    );
    proof::present(
        proof::at(raw, "/value")?,
        "physical",
        &float("42340000", 45.0),
        Some(true),
    )?;
    proof::present(
        proof::at(fixed, "/boundingBoxScale/value")?,
        "z",
        &float("3f800000", 1.0),
        Some(true),
    )?;
    proof::present(
        proof::at(fixed, "/damageMapNoiseStrength/value")?,
        "burn",
        &float("3f000000", 0.5),
        Some(true),
    )?;
    check_fraction(fraction)?;
    assert_eq!(
        proof::at(base, "/initialDamage/value/damage/value/type")?,
        "base",
        "fieldless authored base survives"
    );
    Ok(())
}

#[test]
fn durability_nested_lifecycle_keeps_repeated_gas_and_actual_link_shapes() -> TestResult {
    let catalog = catalog(&fixture::lifecycle_database())?;
    let row = proof::at(&catalog, "/configurations/durability/rows/0")?;
    let [first, second] = proof::rows(row, "/deathGasComposition/value")? else {
        return Err("two original gas slots".into());
    };
    check_gas(first, &float("00000000", 0.0))?;
    check_gas(second, &float("40000000", 2.0))?;
    assert_ne!(
        proof::at(first, "/sourceUsesRefs")?,
        proof::at(second, "/sourceUsesRefs")?,
        "a repeated gas target does not collapse its original slots"
    );
    let hit = proof::at(row, "/hitManipulationParams/value")?;
    proof::present(
        proof::at(hit, "/ai/value")?,
        "damageMultiplier",
        &float("00000000", 0.0),
        Some(true),
    )?;
    proof::present(
        proof::at(hit, "/player/value")?,
        "damageMultiplier",
        &float("3fa00000", 1.25),
        Some(true),
    )?;
    check_states(&catalog, row)?;
    check_lock(row)
}

#[test]
fn durability_nested_generic_reference_checks_actual_declared_target() -> TestResult {
    let source = fixture::wrong_macro_database(TestType::ReferenceTo("DamageMacro".to_owned()));
    assert_original_macro_target(&source)?;
    check_declared_macro(&catalog(&source)?)?;
    let source = fixture::wrong_macro_database(TestType::Reference);
    assert_original_macro_target(&source)?;
    check_undeclared_macro(&catalog(&source)?)
}

fn assert_original_macro_target(source: &DataForgeBuilder) -> TestResult {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("original Health owner")?.tree()?;
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original Components".into());
    };
    let Some(Original::Struct(health)) = components.get(1) else {
        return Err("original Health component".into());
    };
    let Some(Original::Struct(initial)) = health.field("InitialDamage") else {
        return Err("original InitialDamage".into());
    };
    let Some(Original::Struct(fraction)) = initial.field("Damage") else {
        return Err("original fraction damage".into());
    };
    let Some(Original::Reference(target)) = fraction.field("damageMacro") else {
        return Err("admitted physical macro target".into());
    };
    assert_eq!(
        target.type_name(),
        fixture::WRONG_MACRO_TYPE,
        "physical type"
    );
    assert_eq!(target.record_index(), 1, "actual physical target");
    Ok(())
}

fn check_declared_macro(catalog: &Value) -> TestResult {
    let carrier = macro_carrier(catalog)?;
    let target = record_link(carrier)?;
    assert_eq!(
        proof::at(target, "/status")?,
        "unexpectedTargetType",
        "an admitted incompatible target stays visible with the typed mismatch"
    );
    proof::present(target, "targetRecordIndex", &json!(1), Some(true))?;
    proof::present(
        target,
        "targetType",
        &json!(fixture::WRONG_MACRO_TYPE),
        Some(true),
    )?;
    proof::present(
        target,
        "guid",
        &json!(common::guid(fixture::WRONG_MACRO)),
        Some(true),
    )?;
    let uses = proof::uses(catalog, carrier)?;
    assert!(
        uses.iter()
            .any(|used| selector(used, "dataforgeValue", "$target")),
        "the actual Reference target declaration was consulted"
    );
    assert!(
        uses.iter().any(|used| {
            selector(used, "dataforgeRecordHeader", "sourceType")
                && used.pointer("/location/indices") == Some(&json!(["1"]))
        }),
        "the actual target type header contributes"
    );
    assert!(
        uses.iter()
            .any(|used| selector(used, "dataforgeStructDeclaration", "parent")),
        "the inspected target ancestry contributes"
    );
    Ok(())
}

fn check_undeclared_macro(catalog: &Value) -> TestResult {
    let carrier = macro_carrier(catalog)?;
    assert_eq!(
        proof::at(record_link(carrier)?, "/status")?,
        "present",
        "an unconstrained target accepts the physically admitted Record"
    );
    let uses = proof::uses(catalog, carrier)?;
    assert!(
        uses.iter()
            .any(|used| selector(used, "dataforgeValue", "$target")),
        "the actual unconstrained declaration was consulted"
    );
    assert!(
        !uses
            .iter()
            .any(|used| selector(used, "dataforgeStructDeclaration", "parent")),
        "an unconstrained target uses no ancestry comparison"
    );
    Ok(())
}

fn macro_carrier(catalog: &Value) -> TestResult<&Value> {
    let row = proof::at(catalog, "/configurations/durability/rows/0")?;
    proof::at(row, "/initialDamage/value/damage/value/value/damageMacro")
}

fn selector(used: &Value, namespace: &str, field: &str) -> bool {
    used.get("role").and_then(Value::as_str) == Some("selector")
        && used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
        && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
}

fn catalog(source: &DataForgeBuilder) -> TestResult<Value> {
    let (_folder, archive) = common::open(source, DcbOrigin::Encrypted)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn check_fraction(row: &Value) -> TestResult {
    let fraction = proof::at(row, "/initialDamage/value/damage/value")?;
    assert_eq!(
        proof::at(fraction, "/type")?,
        "fraction",
        "the selected authored fraction form"
    );
    let values = proof::at(fraction, "/value")?;
    proof::present(
        values,
        "healthFraction",
        &float("3f000000", 0.5),
        Some(true),
    )?;
    let target = record_link(proof::at(values, "/damageMacro")?)?;
    proof::present(
        target,
        "guid",
        &json!(common::guid(fixture::MACRO)),
        Some(true),
    )?;
    proof::present(target, "targetType", &json!("DamageMacro"), Some(true))?;
    Ok(())
}

fn check_gas(entry: &Value, expected: &Value) -> TestResult {
    let values = proof::at(entry, "/value")?;
    proof::present(values, "mass", expected, Some(true))?;
    let target = record_link(proof::at(values, "/gas")?)?;
    proof::present(
        target,
        "guid",
        &json!(common::guid(fixture::GAS)),
        Some(true),
    )?;
    proof::present(
        target,
        "targetType",
        &json!("BreathableGasParams"),
        Some(true),
    )?;
    proof::present(target, "targetRecordIndex", &json!(1), Some(true))?;
    assert_eq!(
        proof::at(target, "/status")?,
        "present",
        "the physically admitted gas target"
    );
    Ok(())
}

fn check_states(catalog: &Value, row: &Value) -> TestResult {
    let [first, later] = proof::rows(row, "/healthLevelStates/value")? else {
        return Err("two original Health states".into());
    };
    let first = proof::at(first, "/value")?;
    let later = proof::at(later, "/value")?;
    proof::present(first, "geometryTag", &json!("fixture-repeat"), Some(true))?;
    proof::present(later, "geometryTag", &json!("fixture-repeat"), Some(true))?;
    let target = record_link(proof::at(first, "/effectTrigger")?)?;
    proof::present(
        target,
        "guid",
        &json!(common::guid(fixture::TAG)),
        Some(true),
    )?;
    let missing = record_link(proof::at(first, "/effectTag")?)?;
    assert_eq!(
        proof::at(missing, "/status")?,
        "dangling",
        "the authored missing link remains visible"
    );
    let guid = proof::present(
        missing,
        "guid",
        &json!(common::guid(fixture::MISSING_TAG)),
        Some(true),
    )?;
    assert!(
        proof::uses(catalog, guid)?.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("payload")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$error.guid")
        }),
        "the retained GUID is the original failed reference parameter"
    );
    proof::cell(missing, "targetRecordIndex", "absent")?;
    proof::cell(later, "effectTag", "null")?;
    Ok(())
}

fn check_lock(row: &Value) -> TestResult {
    let lock = proof::at(row, "/interactionLocks/value/0/value")?;
    proof::present(lock, "minHealthRatio", &float("00000000", 0.0), Some(true))?;
    let interaction = proof::cell(lock, "interaction", "present")?;
    let link = proof::at(interaction, "/value")?;
    assert_eq!(
        link.get("type").and_then(Value::as_str),
        Some("weakReference"),
        "the weak pointer keeps its actual storage"
    );
    let owner = proof::at(link, "/value")?;
    proof::present(owner, "targetRecordIndex", &json!(0), Some(true))?;
    let target = proof::at(owner, "/target")?;
    assert_eq!(
        target.get("type").and_then(Value::as_str),
        Some("instance"),
        "the weak target is inside this physical Record"
    );
    proof::present(
        target,
        "location",
        &json!("fixtureInteractionTarget"),
        Some(true),
    )?;
    Ok(())
}

fn record_link(cell: &Value) -> TestResult<&Value> {
    assert_eq!(
        proof::at(cell, "/state")?,
        "present",
        "the authored link is present"
    );
    assert_eq!(
        proof::at(cell, "/value/type")?,
        "recordReference",
        "a Record Reference keeps its storage type"
    );
    proof::at(cell, "/value/value")
}

fn float(bits: &str, finite: f32) -> Value {
    json!({"rawBits": bits, "finite": finite})
}
