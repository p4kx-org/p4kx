// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, ExportDiagnosticStage, ExportHealthStatus, HealthText, Kind};

use crate::health_origin_fixture::{self, Fixture, Input, TestResult};
use crate::mission_property_remaining_fixture::{self as fixture, Case};
use crate::mission_property_remaining_origins;
use crate::mission_property_remaining_source::{self as source, Original};

struct Checked {
    input: Fixture,
    actual: Original,
    case: Case,
    kind: Input,
}

#[test]
fn broker_callbacks_keep_actual_owned_mapper_and_context_error_receipts() -> TestResult {
    let mut inputs = Vec::new();
    for case in [
        Case::Investigation,
        Case::Option,
        Case::Clue,
        Case::Organization,
        Case::Spawn,
    ] {
        for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
            let input = fixture::open_broker(case, kind)?;
            let actual = source::check_broker(&input, case)?;
            inputs.push(Checked {
                input,
                actual,
                case,
                kind,
            });
        }
    }
    for checked in inputs {
        run(&checked)?;
    }
    Ok(())
}

fn run(checked: &Checked) -> TestResult {
    let (built, document) = checked.input.diagnostic()?;
    let health = built.summary().export_health();
    assert_eq!(
        health.status,
        ExportHealthStatus::Incomplete,
        "actual required Broker guard failure"
    );
    let example = example(&built, checked.case)?;
    body(example, checked);
    retained(&document, &built)?;
    mission_property_remaining_origins::check(&document, example, &checked.actual)?;
    assert!(
        crate::broker_flow_graph::projections(&document)?.is_empty(),
        "no partial failed Broker row"
    );
    assert!(
        matches!(
            p4kx_export::build_catalog_from(
                &checked.input.archive,
                &checked.input.entry,
                &health_origin_fixture::options(),
            ),
            Err(CatalogError::IncompleteExport(_))
        ),
        "same required Broker failure refuses Normal"
    );
    Ok(())
}

fn example(
    built: &p4kx_export::BuiltCatalog,
    case: Case,
) -> TestResult<&p4kx_export::ExportDiagnosticExample> {
    let health = built.summary().export_health();
    let matching: Vec<_> = health
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.kind == Some(Kind::MissionBrokerEntries)
                && diagnostic.stage == ExportDiagnosticStage::DerivedProjection
                && diagnostic.code == case.code()
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "exact selected Broker cause, not a generic incomplete substitute"
    );
    let diagnostic = matching.first().ok_or("actual Broker diagnostic")?;
    assert_eq!(diagnostic.count, 1, "one physical Broker source");
    assert_eq!(
        diagnostic.examples().len(),
        1,
        "the original first failing guard"
    );
    diagnostic
        .examples()
        .first()
        .ok_or_else(|| "actual Broker example".into())
}

fn body(example: &p4kx_export::ExportDiagnosticExample, checked: &Checked) {
    assert_eq!(
        example.record_index().value(),
        Some(&0),
        "original physical owner"
    );
    let guid = health_origin_fixture::guid(crate::scoped_fixture::BROKER);
    assert_eq!(
        example.record_guid().value().map(HealthText::as_str),
        Some(guid.as_str()),
        "authentic Broker header"
    );
    assert_eq!(
        example.field_text().value(),
        Some(checked.case.field()),
        "same actual domain field"
    );
    assert_eq!(
        example.source_type_text().value(),
        Some(checked.case.source_type()),
        "same actual concrete source"
    );
    assert_eq!(
        example
            .source_type_text()
            .origin()
            .contains_encrypted_source(),
        checked.kind.marker(),
        "own actual origin"
    );
    assert!(
        example.source_type_text().origin().source_set_complete(),
        "complete original body recipe"
    );
    let marker = if checked.case.stored_field().is_some() {
        checked.kind.marker()
    } else {
        Some(false)
    };
    assert_eq!(
        example.field_text().origin().contains_encrypted_source(),
        marker,
        "own original field or independent operation origin"
    );
    assert!(
        example.field_text().origin().source_set_complete(),
        "complete original field/control recipe"
    );
    assert!(
        example.target_guid().value().is_none(),
        "no fabricated target GUID"
    );
}

fn retained(document: &serde_json::Value, built: &p4kx_export::BuiltCatalog) -> TestResult {
    let rows = document
        .pointer("/definitions/missionBrokerEntries/rows")
        .and_then(serde_json::Value::as_array)
        .ok_or("canonical retained Broker owners")?;
    assert_eq!(
        rows.len(),
        1,
        "the independently readable actual Flow owner remains emitted in D"
    );
    let row = rows.first().ok_or("actual physical Broker definition")?;
    assert_eq!(
        row.pointer("/guid/value"),
        Some(&serde_json::json!(health_origin_fixture::guid(
            crate::scoped_fixture::BROKER
        ))),
        "the same original physical Broker is retained"
    );
    let health = built
        .summary()
        .export_health()
        .kinds()
        .iter()
        .find(|row| row.kind == Kind::MissionBrokerEntries)
        .ok_or("actual Broker health")?;
    assert_eq!(
        (
            health.source_records.selected,
            health.source_records.read_successful,
            health.source_records.projected,
            health.source_records.omitted_after_projection_failure
        ),
        (1, 1, 1, 0),
        "the successful D source projection is separate from its required C failure"
    );
    assert_eq!(
        health.failed_derived_projections, 1,
        "the original C guard failure remains mandatory"
    );
    crate::broker_flow_origins::payload_closure(row, built.summary().source_inventory())
}
