// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{
    BuiltCatalog, CatalogError, ExportDiagnosticCode, ExportDiagnosticStage, ExportHealthStatus,
    HealthText, Kind,
};
use serde_json::Value;

use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Input, TestResult};
use crate::mission_property_capture_fixture::{self as fixture, Case};
use crate::mission_property_capture_source as source;
use crate::mission_property_sets_fixture::{self as sets, GENERATOR};

#[test]
fn unreadable_generator_overrides_keep_the_actual_guard_receipt() -> TestResult {
    run(Case::OverrideArray)
}

#[test]
fn unreadable_generator_property_values_keep_the_actual_guard_receipt() -> TestResult {
    run(Case::PropertyPointer)
}

#[test]
fn readable_empty_generator_sets_remain_successful() -> TestResult {
    let input = fixture::open(Case::Empty, Input::Plain)?;
    source::check(&input, Case::Empty)?;
    let built = p4kx_export::build_catalog_from(
        &input.archive,
        &input.entry,
        &health_origin_fixture::options(),
    )?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Complete,
        "genuine empty declarations retain successful source and projection"
    );
    assert!(
        built.summary().failures().is_empty(),
        "no invented source failure"
    );
    let document: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        sets::rows(&document)?.len(),
        4,
        "handler, contract and both actual subcontracts"
    );
    Ok(())
}

fn run(case: Case) -> TestResult {
    for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let input = fixture::open(case, kind)?;
        let ordinal = source::check(&input, case)?.ok_or("actual unreadable source ordinal")?;
        let (built, document) = input.diagnostic()?;
        check(&built, &document, case, kind, ordinal)?;
        assert!(
            matches!(
                p4kx_export::build_catalog_from(
                    &input.archive,
                    &input.entry,
                    &health_origin_fixture::options()
                ),
                Err(CatalogError::IncompleteExport(_))
            ),
            "the same genuine required source failure refuses Normal publication"
        );
    }
    Ok(())
}

fn check(
    built: &BuiltCatalog,
    document: &Value,
    case: Case,
    kind: Input,
    ordinal: u64,
) -> TestResult {
    let health = built.summary().export_health();
    assert_eq!(
        health.status,
        ExportHealthStatus::Incomplete,
        "a readable tree can hold a required unreadable value"
    );
    assert_eq!(
        built.summary().failures().len(),
        1,
        "one original failed Generator, no partial output"
    );
    let example = diagnostic(built, case)?;
    assert_eq!(
        example.field_text().origin().contains_encrypted_source(),
        kind.marker(),
        "per-occurrence origin"
    );
    assert!(
        example.field_text().origin().source_set_complete(),
        "actual creator receipt is complete"
    );
    let cell = serde_json::to_value(example.field())?;
    let used = origin::uses(document, &cell)?;
    assert!(
        used.iter().any(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeValue")
                && used.pointer("/location/indices")
                    == Some(&serde_json::json!(["0", ordinal.to_string()]))
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$fieldName")
                && used.get("role").and_then(Value::as_str) == Some("conversion")
        }),
        "the actual failed declaration ordinal contributes its original name"
    );
    assert!(
        sets::rows(document)?.is_empty(),
        "failed Generator emits no partial property sets"
    );
    assert!(
        crate::catalog_contracts::v3::rows(document)?.is_empty(),
        "failed Generator emits no partial contracts"
    );
    origin::closure(document)
}

fn diagnostic(
    built: &BuiltCatalog,
    case: Case,
) -> TestResult<&p4kx_export::ExportDiagnosticExample> {
    let matching: Vec<_> = built
        .summary()
        .export_health()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.kind == Some(Kind::Contracts)
                && diagnostic.stage == ExportDiagnosticStage::Projection
                && diagnostic.code == ExportDiagnosticCode::UnreadableSource
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "precise original field cause survives through the required failure"
    );
    let diagnostic = matching.first().ok_or("original guard diagnostic")?;
    assert_eq!(diagnostic.count, 1, "one failed original Record");
    let examples = diagnostic.examples();
    assert_eq!(examples.len(), 1, "one retained genuine guard occurrence");
    let example = examples.first().ok_or("original guard example")?;
    assert_eq!(
        example.record_index().value(),
        Some(&0),
        "the same actual owner"
    );
    assert_eq!(
        example.record_guid().value().map(HealthText::as_str),
        Some(GENERATOR),
        "authentic owner header"
    );
    assert_eq!(
        example.field_text().value(),
        Some(case.field()),
        "existing exact domain field spelling"
    );
    assert!(
        example.source_type_text().value().is_none(),
        "the domain error has no sourceType member"
    );
    assert!(
        example.target_guid().value().is_none(),
        "no fabricated target from malformed pointer words"
    );
    Ok(example)
}
