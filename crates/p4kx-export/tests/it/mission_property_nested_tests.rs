// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{
    CatalogError, ExportDiagnosticCode, ExportDiagnosticExample, ExportDiagnosticStage,
    ExportHealthStatus, Kind,
};
use serde_json::{Value, json};

use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Input, TestResult};
use crate::mission_property_nested_fixture::{self as fixture, Case};
use crate::mission_property_nested_source::{self as source, Original};
use crate::mission_property_sets_fixture as sets;

#[test]
fn unknown_generator_property_type_keeps_its_actual_original_type() -> TestResult {
    run(Case::UnknownValue)
}

#[test]
fn unknown_scoped_filter_keeps_its_actual_original_type() -> TestResult {
    run(Case::UnknownRelative)
}

#[test]
fn unreadable_nested_search_keeps_its_actual_original_field() -> TestResult {
    run(Case::UnreadableSearch)
}

fn run(case: Case) -> TestResult {
    for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let input = fixture::open(case, kind)?;
        let actual = source::check(&input, case)?;
        let (built, document) = input.diagnostic()?;
        check(&built, &document, case, kind, &actual)?;
        assert!(
            matches!(
                p4kx_export::build_catalog_from(
                    &input.archive,
                    &input.entry,
                    &health_origin_fixture::options()
                ),
                Err(CatalogError::IncompleteExport(_))
            ),
            "the same original required failure refuses Normal"
        );
    }
    Ok(())
}

fn check(
    built: &p4kx_export::BuiltCatalog,
    document: &Value,
    case: Case,
    kind: Input,
    actual: &Original,
) -> TestResult {
    let health = built.summary().export_health();
    assert_eq!(
        health.status,
        ExportHealthStatus::Incomplete,
        "required interpretation remains failed"
    );
    assert_eq!(
        built.summary().failures().len(),
        1,
        "the same one original Generator is omitted"
    );
    let code = if matches!(case, Case::UnreadableSearch) {
        ExportDiagnosticCode::UnreadableSource
    } else {
        ExportDiagnosticCode::UnsupportedValue
    };
    let matching: Vec<_> = health
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.kind == Some(Kind::Contracts)
                && diagnostic.stage == ExportDiagnosticStage::Projection
                && diagnostic.code == code
        })
        .collect();
    assert_eq!(matching.len(), 1, "exact nested original failure category");
    let diagnostic = matching.first().ok_or("original nested diagnostic")?;
    assert_eq!(diagnostic.count, 1, "one original failed Record");
    assert_eq!(
        diagnostic.examples().len(),
        1,
        "the original first guard remains selected"
    );
    body(
        document,
        diagnostic
            .examples()
            .first()
            .ok_or("original nested example")?,
        case,
        kind,
        actual,
    )?;
    assert!(sets::rows(document)?.is_empty(), "no partial source sets");
    assert!(
        crate::catalog_contracts::v3::rows(document)?.is_empty(),
        "no partial derived contracts"
    );
    Ok(())
}

fn body(
    document: &Value,
    example: &ExportDiagnosticExample,
    case: Case,
    kind: Input,
    actual: &Original,
) -> TestResult {
    assert_eq!(
        example.record_index().value(),
        Some(&0),
        "the same actual Generator owner"
    );
    assert_eq!(
        example.field_text().value(),
        Some(case.field()),
        "existing exact domain label"
    );
    assert_eq!(
        example.source_type_text().value(),
        Some(case.source_type()),
        "actual original concrete type"
    );
    assert!(
        example.target_guid().value().is_none(),
        "no fabricated target"
    );
    assert_eq!(
        example
            .source_type_text()
            .origin()
            .contains_encrypted_source(),
        kind.marker(),
        "actual type origin"
    );
    assert!(
        example.source_type_text().origin().source_set_complete(),
        "actual type receipt is complete"
    );
    let cell = serde_json::to_value(example.source_type())?;
    let used = origin::uses(document, &cell)?;
    assert_eq!(
        used.len(),
        1,
        "only the actual concrete type contributes to sourceType"
    );
    coordinate(
        used.first().ok_or("genuine source type use")?,
        actual.source_type_ordinal,
        "$type",
        "selector",
    );
    field(document, example, case, kind, actual)?;
    origin::closure(document)
}

fn field(
    document: &Value,
    example: &ExportDiagnosticExample,
    case: Case,
    kind: Input,
    actual: &Original,
) -> TestResult {
    let marker = if matches!(case, Case::UnreadableSearch) {
        kind.marker()
    } else {
        Some(false)
    };
    assert_eq!(
        example.field_text().origin().contains_encrypted_source(),
        marker,
        "static operation labels and original field names have their own distinct origins"
    );
    assert!(
        example.field_text().origin().source_set_complete(),
        "actual field interpretation is complete"
    );
    let cell = serde_json::to_value(example.field())?;
    let used = origin::uses(document, &cell)?;
    if let Some(ordinal) = actual.failed_value_ordinal {
        assert!(
            used.iter()
                .any(|used| is_coordinate(used, ordinal, "$fieldName", "conversion")),
            "the genuine unreadable declaration contributes at its actual original ordinal"
        );
    } else {
        assert!(
            used.is_empty(),
            "a static unknown-type operation label claims no original bytes"
        );
    }
    Ok(())
}

fn coordinate(used: &Value, ordinal: u64, field: &str, role: &str) {
    assert!(
        is_coordinate(used, ordinal, field, role),
        "exact original typed coordinate and role: {used}"
    );
}

fn is_coordinate(used: &Value, ordinal: u64, field: &str, role: &str) -> bool {
    used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
        && used.pointer("/location/indices") == Some(&json!(["0", ordinal.to_string()]))
        && used.pointer("/location/field") == Some(&json!(field))
        && used.get("role") == Some(&json!(role))
}
