// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{
    CatalogError, ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage,
    ExportHealth, ExportStageStatus, Kind,
};
use p4kx_extraction::DataType;
use p4kx_testkit::TestType;
use serde_json::{Value, json};

use crate::c2_sequence_storage_fixture::{self as fixture, Case};
use crate::c2_sequence_storage_source as source;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::gameplay_v3_original as original;
use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_support as support;

#[test]
fn readable_class_array_in_sequence_cannot_receive_healthy_normal_publication() -> TestResult {
    let database = fixture::database(TestType::Class(schema::BASE.to_owned()), Case::TwoOwned);
    source::check(&database, DataType::Class, Case::TwoOwned)?;
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    match p4kx_export::build_catalog(&archive, &common::options()) {
        Err(CatalogError::IncompleteExport(refusal)) => diagnosis(refusal.health()),
        Ok(built) => Err(format!(
            "readable Class declaration received a normal catalog with {:?} health",
            built.summary().export_health().status
        )
        .into()),
        Err(error) => Err(format!("unexpected declaration failure: {error}").into()),
    }
}

#[test]
fn diagnostic_class_array_retains_every_original_occurrence_and_reports_wrong_storage() -> TestResult
{
    let database = fixture::database(TestType::Class(schema::BASE.to_owned()), Case::TwoOwned);
    source::check(&database, DataType::Class, Case::TwoOwned)?;
    let (built, catalog) = support::diagnostic(&database)?;
    original::check(&catalog, &database, 0)?;
    retained(&catalog)?;
    diagnosis(built.summary().export_health())
}

fn retained(catalog: &Value) -> TestResult {
    let graph = support::at(support::row(catalog, 0)?, "/displayTransformation/value")?;
    assert_eq!(
        support::at(graph, "/root/nodeIndex")?,
        &json!(0),
        "the original root occurrence is retained"
    );
    assert_eq!(
        support::at(graph, "/nodes")?.as_array().map(Vec::len),
        Some(3),
        "the Sequence and both equal empty Base occurrences remain independent"
    );
    for (slot, node) in [(0, 1), (1, 2)] {
        let child = format!("/nodes/0/payload/value/transformations/value/{slot}/value/nodeIndex");
        assert_eq!(
            support::at(graph, &child)?,
            &json!(node),
            "each original ordered slot links its independent occurrence"
        );
        assert_eq!(
            support::at(graph, &format!("/nodes/{node}/sourceType/value"))?,
            &json!(schema::BASE),
            "the original child type remains exact"
        );
        assert_eq!(
            support::at(graph, &format!("/nodes/{node}/payload/value/type"))?,
            &json!("emptyBase"),
            "the readable empty payload remains present"
        );
    }
    Ok(())
}

#[test]
fn valid_strong_pointer_sequences_and_null_or_empty_neighbors_remain_healthy() -> TestResult {
    for case in [Case::TwoOwned, Case::Empty, Case::NullChild, Case::NullRoot] {
        let database = fixture::database(TestType::StrongPointer(schema::BASE.to_owned()), case);
        source::check(&database, DataType::StrongPointer, case)?;
        let (built, catalog) = support::normal(&database)?;
        assert!(
            built.summary().export_health().is_complete(),
            "every genuine valid neighbor remains healthy"
        );
        original::check(&catalog, &database, 0)?;
        neighbor(&catalog, case)?;
    }
    Ok(())
}

fn neighbor(catalog: &Value, case: Case) -> TestResult {
    let row = support::row(catalog, 0)?;
    if let Some(expected) = fixture::node_count(case) {
        assert_eq!(
            support::at(row, "/displayTransformation/value/nodes")?
                .as_array()
                .map(Vec::len),
            Some(expected),
            "null slots and empty sequences do not invent owned nodes"
        );
    } else {
        support::state(support::at(row, "/displayTransformation")?, "null")?;
    }
    Ok(())
}

fn diagnosis(health: &ExportHealth) -> TestResult {
    assert!(
        !health.is_complete(),
        "readable wrong storage cannot be healthy"
    );
    let kind = health
        .kind(Kind::GameplayProperties)
        .ok_or("actual gameplay kind health")?;
    assert_eq!(
        kind.read,
        ExportStageStatus::Complete,
        "the original canonical read succeeded"
    );
    assert_eq!(
        kind.projection,
        ExportStageStatus::Incomplete,
        "the declaration mismatch is explicit projection health"
    );
    assert!(
        health.diagnostics().iter().any(|entry| {
            entry.kind == Some(Kind::GameplayProperties)
                && entry.code == ExportDiagnosticCode::MalformedSource
                && entry.stage == ExportDiagnosticStage::Projection
                && entry.severity == ExportDiagnosticSeverity::Error
                && entry.examples().iter().any(|example| {
                    example.record_index().value() == Some(&0)
                        && example.field_text().value()
                            == Some("displayTransformation.transformations")
                        && example.source_type_text().value() == Some(schema::SEQUENCE)
                })
        }),
        "the diagnostic names the physical owner and wrong original Sequence field"
    );
    Ok(())
}
