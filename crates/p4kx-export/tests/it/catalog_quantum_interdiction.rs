// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_quantum_interdiction_fixture.rs"]
mod fixture;

const ROWS: &str = "/configurations/quantumInterdiction/rows";

#[test]
fn quantum_interdiction_preserves_jammer_pulse_weak_interactions_and_visual_graph() -> TestResult {
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&fixture::database(false), origin)?;
        let built = build_catalog(&archive, &common::options())?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let schema = p4kx_export::catalog_schema();
        let validator = jsonschema::validator_for(&schema)?;
        validator
            .validate(&catalog)
            .map_err(|error| error.to_string())?;
        let row = proof::rows(&catalog, ROWS)?.first().ok_or("QIG row")?;
        proof::present(row, "sourceType", &json!(fixture::DEVICE), Some(encrypted))?;
        proof::present(row, "componentOrdinal", &json!(0), Some(encrypted))?;
        let jammer = value(row, "jammerSettings")?;
        let pulse = value(row, "pulseSettings")?;
        proof::present(
            jammer,
            "sourceType",
            &json!(fixture::JAMMER),
            Some(encrypted),
        )?;
        proof::present(pulse, "sourceType", &json!(fixture::PULSE), Some(encrypted))?;
        number(&catalog, jammer, "jammerRange", 28.0, encrypted)?;
        number(&catalog, pulse, "chargeTimeSeconds", 0.0, encrypted)?;
        number(
            &catalog,
            pulse,
            "activationPhaseDurationSeconds",
            2.0,
            encrypted,
        )?;
        proof::cell(pulse, "cooldownTimeSeconds", "absent")?;
        proof::cell(pulse, "startChargingInteraction", "null")?;
        let switch = proof::cell(row, "mainDeviceSwitchOn", "present")?;
        assert_ne!(
            proof::uses(&catalog, switch)?,
            [] as [&Value; 0],
            "weak interaction retains its original source use"
        );
        let extra = proof::rows(row, "/additionalFields/roots")?;
        assert_eq!(
            extra.len(),
            1,
            "typed fields and raw extras have distinct occurrences"
        );
        proof::present(
            extra.first().ok_or("retained component field")?,
            "name",
            &json!("fixtureExtraField"),
            Some(encrypted),
        )?;
        verify_graph(&catalog, value(row, "visualGraph")?, encrypted)?;
    }
    Ok(())
}

#[test]
fn quantum_interdiction_does_not_accept_a_foreign_declared_jammer_pointer() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(true), DcbOrigin::Plain)?;
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "the foreign nominal declaration prevents complete publication"
    );
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = proof::rows(&catalog, ROWS)?.first().ok_or("QIG row")?;
    proof::cell(row, "jammerSettings", "unsupported")?;
    assert_eq!(
        proof::rows(row, "/additionalFields/roots")?.len(),
        2,
        "the original invalid declaration remains alongside untouched raw extras"
    );
    Ok(())
}

fn verify_graph(catalog: &Value, graph: &Value, encrypted: bool) -> TestResult {
    proof::cell(graph, "effectStrengthLink", "null")?;
    let first = proof::at(graph, "/states/value/0/value")?;
    let animation = value(first, "animationParams")?;
    proof::present(
        animation,
        "fragmentName",
        &json!("fixture"),
        Some(encrypted),
    )?;
    proof::present(animation, "fragmentTag", &json!(""), Some(encrypted))?;
    proof::present(animation, "loop", &json!(false), Some(encrypted))?;
    let effect = value(first, "effectParams")?;
    let tag = value(effect, "entityEffectTag")?;
    proof::present(
        tag,
        "guid",
        &json!(common::guid(fixture::TAG_GUID)),
        Some(encrypted),
    )?;
    proof::cell(effect, "entityEffectTrigger", "null")?;
    verify_transitions(
        catalog,
        proof::rows(first, "/transitions/value")?,
        encrypted,
    )
}

fn verify_transitions(catalog: &Value, transitions: &[Value], encrypted: bool) -> TestResult {
    let [first, second] = transitions else {
        return Err("two ordered transition occurrences".into());
    };
    let first = proof::at(first, "/value")?;
    let second = proof::at(second, "/value")?;
    proof::present(
        first,
        "state",
        &json!("FixtureTargetStateB"),
        Some(encrypted),
    )?;
    proof::present(
        second,
        "state",
        &json!("FixtureTargetStateA"),
        Some(encrypted),
    )?;
    let time = value(first, "animationTransitionType")?;
    let dynamic = value(first, "effectTransitionType")?;
    assert_eq!(
        time.get("kind"),
        Some(&json!("timeBased")),
        "stored timing selector has its concrete variant"
    );
    assert_eq!(
        dynamic.get("kind"),
        Some(&json!("valueBased")),
        "stored context selector has its concrete variant"
    );
    number(catalog, time, "seconds", 0.0, encrypted)?;
    proof::present(
        dynamic,
        "context",
        &json!("FixtureContext"),
        Some(encrypted),
    )?;
    proof::present(
        dynamic,
        "variable",
        &json!("FixtureVariable"),
        Some(encrypted),
    )?;
    verify_future_transition(second, encrypted)
}

fn verify_future_transition(transition: &Value, encrypted: bool) -> TestResult {
    let future = value(transition, "animationTransitionType")?;
    assert_eq!(
        future.get("kind"),
        Some(&json!("authored")),
        "a future variant stays authored and unclassified"
    );
    proof::present(
        future,
        "sourceType",
        &json!(fixture::FUTURE),
        Some(encrypted),
    )?;
    let raw = proof::rows(future, "/additionalFields/roots")?;
    assert_eq!(raw.len(), 1, "unrecognized variant retains authored fields");
    proof::present(
        raw.first().ok_or("retained selector member")?,
        "name",
        &json!("fixtureExtension"),
        Some(encrypted),
    )?;
    proof::cell(transition, "effectTransitionType", "null")?;
    Ok(())
}

fn value<'a>(row: &'a Value, field: &str) -> TestResult<&'a Value> {
    proof::at(proof::cell(row, field, "present")?, "/value")
}

fn number(catalog: &Value, row: &Value, field: &str, number: f32, encrypted: bool) -> TestResult {
    let finite: Value = serde_json::from_slice(&serde_json::to_vec(&number)?)?;
    let cell = proof::present(
        row,
        field,
        &json!({"type":"f32","finite":finite,"rawBits":format!("{:08x}", number.to_bits())}),
        Some(encrypted),
    )?;
    assert_ne!(
        proof::uses(catalog, cell)?,
        [] as [&Value; 0],
        "stored number retains its original source use"
    );
    Ok(())
}
