// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{
    BuiltCatalog, CatalogError, CatalogOptions, CatalogPublicationMode, Compression,
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportHealthStatus,
    HealthText, Kind,
};
use serde_json::Value;

use crate::flow_final_fixture::{self as fixture, Case};
use crate::flow_final_graph::Graph;
use crate::flow_final_meaning;
use crate::flow_final_original::Original;
use crate::flow_final_raw_graph;
use crate::flow_final_source as source;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

/// Prefix, external owned value, its marker and the external numeric field are outside Flow.
const OUTSIDE_OCCURRENCES: u64 = 4;

struct Checked {
    fixture: Fixture,
    original: Original,
    input: Input,
}

#[test]
fn readable_unknown_flow_retains_all19_types_and_direct_origins_in_diagnostic() -> TestResult {
    let inputs = originals(
        Case::Unknown,
        &[Input::Plain, Input::Encrypted, Input::Standalone],
    )?;
    for input in inputs {
        run(&input, Case::Unknown, ExportHealthStatus::Incomplete)?;
    }
    Ok(())
}

#[test]
fn actual_flow_nonfinite_is_covered_without_waiving_the_outside_owner_value() -> TestResult {
    let inside = originals(
        Case::KnownNan,
        &[Input::Plain, Input::Encrypted, Input::Standalone],
    )?;
    let outside = originals(Case::OutsideNan, &[Input::Plain])?;
    for input in inside {
        run(&input, Case::KnownNan, ExportHealthStatus::Complete)?;
    }
    for input in outside {
        run(&input, Case::OutsideNan, ExportHealthStatus::Incomplete)?;
    }
    Ok(())
}

#[test]
fn equal_original_occurrences_keep_null_and_exact_error_slots_between_them() -> TestResult {
    let inputs = originals(Case::Holes, &[Input::Plain, Input::Encrypted])?;
    for input in inputs {
        run(&input, Case::Holes, ExportHealthStatus::Incomplete)?;
    }
    Ok(())
}

#[test]
fn absent_declaration_actual_null_and_present_empty_flow_are_distinct() -> TestResult {
    let mut cases = Vec::new();
    for case in [Case::Absent, Case::Null, Case::Empty] {
        cases.push((case, originals(case, &[Input::Plain])?));
    }
    for (case, inputs) in cases {
        for input in inputs {
            run(&input, case, ExportHealthStatus::Complete)?;
        }
    }
    Ok(())
}

#[test]
fn duplicate_root_declarations_retain_shadowed_unknown_facts_without_reselecting_them() -> TestResult
{
    let inputs = originals(Case::Duplicate, &[Input::Plain, Input::Encrypted])?;
    for input in inputs {
        run(&input, Case::Duplicate, ExportHealthStatus::Complete)?;
    }
    Ok(())
}

fn originals(case: Case, kinds: &[Input]) -> TestResult<Vec<Checked>> {
    let mut result = Vec::new();
    for input in kinds {
        let fixture = fixture::open(case, *input)?;
        let original = source::check(&fixture, case, *input)?;
        let count: usize = original
            .configurations
            .iter()
            .map(|value| value.nodes.len())
            .sum();
        assert_eq!(
            original.visited,
            u64::try_from(count)? + OUTSIDE_OCCURRENCES,
            "full original DFS counted the non-flow source facts independently"
        );
        result.push(Checked {
            fixture,
            original,
            input: *input,
        });
    }
    Ok(result)
}

fn run(input: &Checked, case: Case, expected: ExportHealthStatus) -> TestResult {
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    if expected == ExportHealthStatus::Incomplete {
        normal_refusal(input, &options);
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    }
    let built =
        p4kx_export::build_catalog_from(&input.fixture.archive, &input.fixture.entry, &options)?;
    assert_eq!(
        built.summary().export_health().status,
        expected,
        "source retention does not certify semantic support or publication"
    );
    failure(&built, case)?;
    output(&built, input, case)?;
    writers(input, case, expected, &options, built.json())
}

fn output(built: &BuiltCatalog, input: &Checked, case: Case) -> TestResult {
    let catalog: Value = serde_json::from_slice(built.json())?;
    let inventory = built.summary().source_inventory();
    for (index, original) in input.original.configurations.iter().enumerate() {
        let graph = Graph::new(
            &catalog,
            inventory,
            input.input,
            index,
            input.original.configurations.len(),
        )?;
        flow_final_raw_graph::check(&graph, original, inventory, input.input)?;
        flow_final_meaning::check(&graph, original, case, index)?;
    }
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "the complete producer document satisfies the actual generated canonical schema"
    );
    Ok(())
}

fn writers(
    input: &Checked,
    case: Case,
    expected: ExportHealthStatus,
    options: &CatalogOptions<'_>,
    baseline: &[u8],
) -> TestResult {
    for compression in [Compression::None, Compression::ZSTD] {
        let (summary, bytes) = p4kx_export::write_catalog_from(
            &input.fixture.archive,
            &input.fixture.entry,
            options,
            compression,
            || Ok(Vec::new()),
        )?;
        assert_eq!(
            summary.export_health().status,
            expected,
            "same publication result for each actual writer"
        );
        let plain = match compression {
            Compression::None => bytes,
            Compression::Zstd { .. } => zstd::stream::decode_all(bytes.as_slice())?,
        };
        assert_eq!(
            plain, baseline,
            "whole final catalog, not a stripped view, remains exact"
        );
        let catalog: Value = serde_json::from_slice(&plain)?;
        for (index, original) in input.original.configurations.iter().enumerate() {
            let graph = Graph::new(
                &catalog,
                summary.source_inventory(),
                input.input,
                index,
                input.original.configurations.len(),
            )?;
            flow_final_raw_graph::check(&graph, original, summary.source_inventory(), input.input)?;
            flow_final_meaning::check(&graph, original, case, index)?;
        }
    }
    Ok(())
}
fn normal_refusal(input: &Checked, options: &CatalogOptions<'_>) {
    assert!(
        matches!(
            p4kx_export::build_catalog_from(&input.fixture.archive, &input.fixture.entry, options),
            Err(CatalogError::IncompleteExport(_))
        ),
        "required failure remains strict Normal refusal"
    );
    let mut created = false;
    let result = p4kx_export::write_catalog_from(
        &input.fixture.archive,
        &input.fixture.entry,
        options,
        Compression::None,
        || {
            created = true;
            Ok(Vec::new())
        },
    );
    assert!(
        matches!(result, Err(CatalogError::IncompleteExport(_))),
        "normal writer retains the same required failure"
    );
    assert!(
        !created,
        "required failure is established before destination creation"
    );
}

fn outside_failure(built: &BuiltCatalog) {
    let matching: Vec<_> = built
        .summary()
        .export_health()
        .diagnostics()
        .iter()
        .filter(|item| {
            item.kind == Some(Kind::ContractTemplates)
                && item.stage == ExportDiagnosticStage::Projection
                && item.code == ExportDiagnosticCode::UnsupportedValue
                && item.severity == ExportDiagnosticSeverity::Error
                && item.examples().iter().any(|example| {
                    example.record_index().value() == Some(&0)
                        && example.record_guid().value().map(HealthText::as_str)
                            == Some(fixture::GUID)
                        && example.field_text().value() == Some("outsideNumber")
                })
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "the uncaptured outside token is the actual blocking reason, not a nominal kind waiver"
    );
    assert_eq!(
        matching.first().map(|diagnostic| diagnostic.count),
        Some(1),
        "exact uncovered outside-token cardinality"
    );
}
fn failure(built: &BuiltCatalog, case: Case) -> TestResult {
    if matches!(case, Case::Unknown) {
        let failures = built.summary().failures();
        assert_eq!(
            failures.len(),
            1,
            "one primary original Record failure remains preserved"
        );
        assert!(
            failures
                .first()
                .ok_or("actual unknown failure")?
                .reason()
                .contains(
                    "missionFlow condition holds the unknown struct MissionFlowCondition_Mystery"
                ),
            "actual unknown type is not reported as a depth limit"
        );
    }
    if matches!(case, Case::OutsideNan) {
        outside_failure(built);
    }
    Ok(())
}
