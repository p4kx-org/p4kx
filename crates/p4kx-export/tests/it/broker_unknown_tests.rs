// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{
    CatalogError, Compression, ExportDiagnosticCode, ExportDiagnosticStage, ExportHealthStatus,
    HealthText, Kind,
};
use serde_json::Value;

use crate::broker_unknown_fixture::{self as fixture, Case, GUID};
use crate::broker_unknown_source as source;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};
use crate::mission_property_remaining_origins;
use crate::mission_property_remaining_source::Original;

struct Checked {
    input: Fixture,
    kind: Input,
    actual: Original,
}

#[test]
fn unsupported_reputation_block_keeps_the_actual_original_type_body() -> TestResult {
    run(Case::Requirements)
}

#[test]
fn unsupported_reputation_term_keeps_the_actual_original_type_body() -> TestResult {
    run(Case::Term)
}

#[test]
fn unsupported_modifier_keeps_the_actual_original_type_body() -> TestResult {
    run(Case::Modifier)
}

fn run(case: Case) -> TestResult {
    let mut checked = Vec::new();
    for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let input = Fixture::open(&fixture::database(Some(case)), kind, Vec::new())?;
        let actual = source::check(&input, case)?;
        checked.push(Checked {
            input,
            kind,
            actual,
        });
    }
    for checked in checked {
        let (built, document) = checked.input.diagnostic()?;
        assert_eq!(
            built.summary().export_health().status,
            ExportHealthStatus::Incomplete,
            "required unsupported interpretation stays incomplete"
        );
        let example = example(&built)?;
        body(example, case, checked.kind);
        mission_property_remaining_origins::check(&document, example, &checked.actual)?;
        retained(&document, &built)?;
        assert!(
            crate::broker_flow_graph::projections(&document)?.is_empty(),
            "no partial failed Broker row"
        );
        refusal(&checked.input);
    }
    Ok(())
}

fn example(built: &p4kx_export::BuiltCatalog) -> TestResult<&p4kx_export::ExportDiagnosticExample> {
    let matching: Vec<_> = built
        .summary()
        .export_health()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.kind == Some(Kind::MissionBrokerEntries)
                && diagnostic.stage == ExportDiagnosticStage::DerivedProjection
                && diagnostic.code == ExportDiagnosticCode::UnsupportedValue
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "exact unsupported first cause rather than a generic failure"
    );
    let diagnostic = matching.first().ok_or("actual unsupported diagnostic")?;
    assert_eq!(
        diagnostic.count, 1,
        "one actual failed physical Broker source"
    );
    assert_eq!(
        diagnostic.examples().len(),
        1,
        "one actual first unsupported guard"
    );
    diagnostic
        .examples()
        .first()
        .ok_or_else(|| "actual unsupported example".into())
}

fn body(example: &p4kx_export::ExportDiagnosticExample, case: Case, kind: Input) {
    assert_eq!(
        example.record_index().value(),
        Some(&0),
        "actual physical owner"
    );
    let guid = health_origin_fixture::guid(GUID);
    assert_eq!(
        example.record_guid().value().map(HealthText::as_str),
        Some(guid.as_str()),
        "actual original header GUID"
    );
    assert_eq!(
        example.field_text().value(),
        Some(case.operation()),
        "unchanged technical operation label"
    );
    assert_eq!(
        example.source_type_text().value(),
        Some(case.source_type()),
        "actual unsupported concrete type"
    );
    assert!(
        example.field_text().origin().source_set_complete(),
        "the independent operation label is complete"
    );
    assert_eq!(
        example.field_text().origin().contains_encrypted_source(),
        Some(false),
        "technical label is independent"
    );
    assert!(
        example.source_type_text().origin().source_set_complete(),
        "complete actual type receipt"
    );
    assert_eq!(
        example
            .source_type_text()
            .origin()
            .contains_encrypted_source(),
        kind.marker(),
        "actual own input origin"
    );
    assert!(
        example.target_guid().value().is_none(),
        "no invented target GUID"
    );
}

fn refusal(input: &Fixture) {
    let options = health_origin_fixture::options();
    assert!(
        matches!(
            p4kx_export::build_catalog_from(&input.archive, &input.entry, &options),
            Err(CatalogError::IncompleteExport(_))
        ),
        "Normal refuses the same required original cause"
    );
    for compression in [Compression::None, Compression::ZSTD] {
        let mut created = false;
        let result = p4kx_export::write_catalog_from(
            &input.archive,
            &input.entry,
            &options,
            compression,
            || {
                created = true;
                Ok(Vec::new())
            },
        );
        assert!(
            matches!(result, Err(CatalogError::IncompleteExport(_))),
            "each writer preserves required refusal"
        );
        assert!(
            !created,
            "no complete-looking prefix or destination is created"
        );
    }
}

#[test]
fn recognized_reputation_and_modifier_keep_their_original_complete_results() -> TestResult {
    let mut checked = Vec::new();
    for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let input = Fixture::open(&fixture::database(None), kind, Vec::new())?;
        source::positive(&input)?;
        checked.push(input);
    }
    for input in checked {
        let built = p4kx_export::build_catalog_from(
            &input.archive,
            &input.entry,
            &health_origin_fixture::options(),
        )?;
        let document: Value = serde_json::from_slice(built.json())?;
        recognized(&document)?;
    }
    Ok(())
}

fn recognized(document: &Value) -> TestResult {
    let rows = crate::broker_flow_graph::projections(document)?;
    assert_eq!(
        rows.len(),
        1,
        "the recognized original Broker remains published"
    );
    let definition =
        crate::broker_flow_graph::definition(document, &health_origin_fixture::guid(GUID))?;
    let row = crate::broker_flow_graph::projection(document, definition)?;
    let owner = crate::flow_owner_graph::Owner::new(definition)?;
    let terms = crate::loot_v3_graph::array(row, "standingRequirements")?;
    let [term] = terms else {
        return Err("the one exact recognized standing expression".into());
    };
    assert_eq!(
        term.pointer("/operator/value").and_then(Value::as_str),
        Some("and"),
        "the same recognized standing operator is retained"
    );
    assert_eq!(
        term.get("comparison"),
        Some(&Value::Null),
        "an And term has no comparison"
    );
    let original = owner.follow(
        term.get("source")
            .ok_or("actual standing term navigation")?,
    )?;
    crate::loot_v3_graph::structure(original, "SReputationMissionRequirementExpression_And")?;
    let modifiers = crate::loot_v3_graph::array(row, "modifiers")?;
    let [modifier] = modifiers else {
        return Err("the exact recognized modifier cardinality".into());
    };
    assert_eq!(
        modifier.pointer("/kind/value").and_then(Value::as_str),
        Some("lawLicense"),
        "the same recognized modifier is retained"
    );
    let original = owner.follow(
        modifier
            .get("source")
            .ok_or("actual original modifier link")?,
    )?;
    crate::loot_v3_graph::structure(original, "MissionModifier_LawLicense")?;
    Ok(())
}

fn retained(document: &Value, built: &p4kx_export::BuiltCatalog) -> TestResult {
    let rows = document
        .pointer("/definitions/missionBrokerEntries/rows")
        .and_then(Value::as_array)
        .ok_or("canonical retained physical Broker definitions")?;
    assert_eq!(
        rows.len(),
        1,
        "the fully readable original Broker Flow owner is retained in D"
    );
    let row = rows.first().ok_or("actual emitted D owner")?;
    assert_eq!(
        row.pointer("/guid/value"),
        Some(&serde_json::json!(health_origin_fixture::guid(GUID))),
        "the same independently proven physical source identity is retained"
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
            health.source_records.read_successful,
            health.source_records.projected,
            health.source_records.omitted_after_projection_failure
        ),
        (1, 1, 0),
        "the D source projection succeeded independently of the failed C interpretation"
    );
    assert_eq!(
        health.failed_derived_projections, 1,
        "the required C interpretation still failed"
    );
    crate::broker_flow_origins::payload_closure(row, built.summary().source_inventory())
}
