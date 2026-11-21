// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use serde_json::{Value, json};

use super::{HEALTH_ONLY, item};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_durability_lifecycle_fixture.rs"]
mod fixture;

#[test]
fn durability_exports_original_flags_resistance_and_distortion_values() -> TestResult {
    let source = fixture::database().record(item(
        HEALTH_ONLY,
        "durability_lifecycle",
        "Misc",
        vec![
            fixture::health(),
            fixture::distortion(),
            fixture::degradation(false),
        ],
    ));
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        proof::at(&catalog, "/exportHealth/status")?,
        "complete",
        "exact nonfinite payload is represented"
    );
    let health = proof::at(&catalog, "/configurations/durability/rows/0")?;
    let distortion = proof::at(&catalog, "/configurations/distortion/rows/0")?;
    proof::present(health, "componentOrdinal", &json!(1), Some(true))?;
    proof::present(health, "repairable", &json!(false), Some(true))?;
    proof::present(health, "salvageable", &json!(true), Some(true))?;
    proof::present(
        health,
        "damageCap",
        &float("00000000", Some(0.0)),
        Some(true),
    )?;
    let delay = proof::present(
        health,
        "explosionDelayTime",
        &float("ff800000", None),
        Some(true),
    )?;
    let mirror = proof::present(health, "mirrorSalvageHit", &json!(false), Some(true))?;
    assert_distinct_payloads(&catalog, delay, mirror)?;
    check_resistances(health)?;
    proof::present(distortion, "componentOrdinal", &json!(2), Some(true))?;
    proof::present(
        distortion,
        "maximum",
        &float("41200000", Some(10.0)),
        Some(true),
    )?;
    proof::present(
        distortion,
        "powerChangeOnlyAtMaxDistortion",
        &json!(false),
        Some(true),
    )?;
    Ok(())
}

#[test]
fn durability_exports_ordered_wear_dirt_wetness_and_curve_values() -> TestResult {
    let source = fixture::database().record(item(
        HEALTH_ONLY,
        "durability_degradation",
        "Misc",
        vec![fixture::degradation(false)],
    ));
    let (_, catalog) = common::build(&source)?;
    let row = proof::at(&catalog, "/configurations/degradation/rows/0")?;
    proof::present(row, "componentOrdinal", &json!(1), Some(false))?;
    proof::present(row, "stopDegradingIfDestroyed", &json!(false), Some(false))?;
    let rows = proof::rows(row, "/accumulators/value")?;
    let [wear, dirt, wetness, repeated] = rows else {
        return Err("four original accumulator occurrences".into());
    };
    check_accumulator_kinds(wear, dirt, wetness, repeated)?;
    let curve = proof::at(wear, "/value/kind/wear/effectCurve/value")?;
    proof::present(curve, "useLut", &json!(false), Some(false))?;
    let points = proof::rows(curve, "/points/value")?;
    let [first, second] = points else {
        return Err("two authored curve points".into());
    };
    proof::present(
        proof::at(first, "/value")?,
        "x",
        &float("3f800000", Some(1.0)),
        Some(false),
    )?;
    proof::present(
        proof::at(second, "/value")?,
        "x",
        &float("00000000", Some(0.0)),
        Some(false),
    )?;
    Ok(())
}

#[test]
fn durability_reports_unsupported_accumulator_without_collapsing_order() -> TestResult {
    let source = fixture::database().record(item(
        HEALTH_ONLY,
        "durability_unknown_accumulator",
        "Misc",
        vec![fixture::degradation(true)],
    ));
    let (_folder, archive) = common::open(&source, DcbOrigin::Plain)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = proof::at(&catalog, "/configurations/degradation/rows/0")?;
    let slots = proof::rows(row, "/accumulators/value")?;
    let [first, unsupported, third, last] = slots else {
        return Err("unhandled accumulator retains its physical slot".into());
    };
    assert_eq!(
        proof::at(first, "/value/kind/type")?,
        "wear",
        "preceding wear remains"
    );
    assert_eq!(
        proof::at(unsupported, "/state")?,
        "unsupported",
        "unknown concrete type is explicit"
    );
    assert!(
        proof::at(unsupported, "/value")?.is_null(),
        "no fabricated type payload"
    );
    assert_eq!(
        proof::at(third, "/value/kind/type")?,
        "wetness",
        "later wetness remains"
    );
    assert_eq!(
        proof::at(last, "/value/kind/type")?,
        "wear",
        "repeated original wear remains"
    );
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "normal publication refuses an unsupported authored accumulator"
    );
    Ok(())
}

fn check_resistances(row: &Value) -> TestResult {
    let resistance = proof::at(row, "/damageResistances/value")?;
    assert_eq!(
        proof::at(resistance, "/type")?,
        "resistances",
        "actual concrete source form"
    );
    proof::present(resistance, "ignoreMeleeDamage", &json!(false), Some(true))?;
    for field in [
        "physical",
        "energy",
        "distortion",
        "thermal",
        "biochemical",
        "stun",
    ] {
        let block = proof::at(resistance, &format!("/{field}/value"))?;
        proof::present(
            block,
            "multiplier",
            &float("3f800000", Some(1.0)),
            Some(true),
        )?;
        proof::present(
            block,
            "threshold",
            &float("00000000", Some(0.0)),
            Some(true),
        )?;
        proof::present(
            block,
            "damageCap",
            &float("40000000", Some(2.0)),
            Some(true),
        )?;
        proof::present(
            block,
            "damageCapLimit",
            &float("40400000", Some(3.0)),
            Some(true),
        )?;
    }
    Ok(())
}

fn check_accumulator_kinds(
    wear: &Value,
    dirt: &Value,
    wetness: &Value,
    repeated: &Value,
) -> TestResult {
    assert_eq!(
        proof::at(wear, "/value/kind/type")?,
        "wear",
        "first authored type"
    );
    assert_eq!(
        proof::at(dirt, "/value/kind/type")?,
        "dirt",
        "second authored type"
    );
    assert_eq!(
        proof::at(wetness, "/value/kind/type")?,
        "wetness",
        "fieldless concrete type"
    );
    assert_eq!(
        proof::at(repeated, "/value/kind/type")?,
        "wear",
        "repeat kept"
    );
    assert_ne!(
        proof::at(wear, "/sourceUsesRefs")?,
        proof::at(repeated, "/sourceUsesRefs")?,
        "each repeated occurrence has distinct physical evidence"
    );
    let facts = proof::at(wear, "/value/kind/wear")?;
    proof::present(
        facts,
        "maxLifetimeHours",
        &float("41200000", Some(10.0)),
        Some(false),
    )?;
    proof::present(facts, "useAsTimer", &json!(false), Some(false))?;
    proof::present(
        proof::at(dirt, "/value/kind")?,
        "initialDirtRatio",
        &float("00000000", Some(0.0)),
        Some(false),
    )?;
    Ok(())
}

fn assert_distinct_payloads(catalog: &Value, first: &Value, second: &Value) -> TestResult {
    let exact = |cell| -> TestResult<Value> {
        let uses = proof::uses(catalog, cell)?;
        let payload = uses
            .into_iter()
            .find(|row| row.get("role").and_then(Value::as_str) == Some("payload"))
            .ok_or("actual typed payload receipt")?;
        assert_eq!(
            proof::at(payload, "/location/namespace")?,
            "dataforgeValue",
            "actual DataForge occurrence"
        );
        assert_eq!(
            proof::at(payload, "/location/indices/0")?,
            "0",
            "physical selected Entity"
        );
        assert_eq!(
            proof::at(payload, "/location/field")?,
            "$value",
            "actual typed leaf read"
        );
        Ok(proof::at(payload, "/location/indices/1")?.clone())
    };
    assert_ne!(
        exact(first)?,
        exact(second)?,
        "source-backed values cannot share a broad row receipt"
    );
    Ok(())
}

fn float(bits: &str, finite: Option<f32>) -> Value {
    json!({"rawBits": bits, "finite": finite})
}
