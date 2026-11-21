// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogError, CatalogOptions, CatalogPublicationMode, Compression, ExportHealthStatus,
};
use serde_json::Value;

use crate::health_origin_fixture::{Fixture, TestResult};
use crate::template_flow_depth_fixture::{self as fixture, Branch, Tail};
use crate::template_flow_depth_source as source;

#[path = "template_flow_wire.rs"]
mod wire;

#[test]
fn sixteen_actual_not_edges_still_have_the_existing_full_positive() -> TestResult {
    complete(Branch::Not, 16, Tail::Known)
}

#[test]
fn seventeen_readable_not_edges_are_not_a_projection_depth_failure() -> TestResult {
    complete(Branch::Not, 17, Tail::Known)
}

#[test]
fn eighteen_readable_not_edges_are_not_a_projection_depth_failure() -> TestResult {
    complete(Branch::Not, 18, Tail::Known)
}

#[test]
fn sixty_four_source_levels_in_and_chain_have_default_json_and_writer_proofs() -> TestResult {
    let edges = 64usize
        .checked_sub(fixture::CONDITION_SOURCE_DEPTH)
        .ok_or("named sixty-four-level source regression")?;
    complete(Branch::And, edges, Tail::Known)
}

#[test]
fn sixty_four_source_levels_in_or_chain_have_default_json_and_writer_proofs() -> TestResult {
    let edges = 64usize
        .checked_sub(fixture::CONDITION_SOURCE_DEPTH)
        .ok_or("named sixty-four-level source regression")?;
    complete(Branch::Or, edges, Tail::Known)
}

#[test]
fn missing_not_and_empty_and_keep_their_distinct_real_values() -> TestResult {
    complete(Branch::Not, 0, Tail::Missing)?;
    complete(Branch::And, 0, Tail::Empty)
}

#[test]
fn a_readable_unknown_flow_stays_a_strict_unknown_struct_failure() -> TestResult {
    unknown(0)
}

#[test]
fn a_late_readable_unknown_flow_is_not_misreported_as_exporter_depth() -> TestResult {
    unknown(17)
}

fn complete(branch: Branch, count: usize, tail: Tail) -> TestResult {
    let input = fixture::open(branch, count, tail)?;
    source::check(&input, branch, count, tail)?;
    let options = CatalogOptions::new(UNIX_EPOCH);
    let built = p4kx_export::build_catalog_from(&input.archive, &input.entry, &options)?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Complete,
        "actual source is fully readable"
    );
    assert!(
        built.summary().failures().is_empty(),
        "no source-backed projection failed"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    output(&catalog, branch, count, tail)?;
    writers(&input, &options, built.json(), branch, count, tail)
}

fn writers(
    input: &Fixture,
    options: &CatalogOptions<'_>,
    baseline: &[u8],
    branch: Branch,
    count: usize,
    tail: Tail,
) -> TestResult {
    for compression in [Compression::None, Compression::ZSTD] {
        let (summary, bytes) = p4kx_export::write_catalog_from(
            &input.archive,
            &input.entry,
            options,
            compression,
            || Ok(Vec::new()),
        )?;
        assert_eq!(
            summary.export_health().status,
            ExportHealthStatus::Complete,
            "full source health"
        );
        let plain = match compression {
            Compression::None => bytes,
            Compression::Zstd { .. } => zstd::stream::decode_all(bytes.as_slice())?,
        };
        assert_eq!(plain, baseline, "whole actual writer document is retained");
        let catalog: Value = serde_json::from_slice(&plain)?;
        output(&catalog, branch, count, tail)?;
    }
    Ok(())
}

fn output(catalog: &Value, branch: Branch, count: usize, tail: Tail) -> TestResult {
    wire::output(catalog, branch, count, tail)
}
fn unknown(count: usize) -> TestResult {
    let input = fixture::open(Branch::Not, count, Tail::Unknown)?;
    source::check(&input, Branch::Not, count, Tail::Unknown)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    let result = p4kx_export::build_catalog_from(&input.archive, &input.entry, &options);
    assert!(
        matches!(result, Err(CatalogError::IncompleteExport(_))),
        "unknown structs remain strict Normal refusal"
    );
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog_from(&input.archive, &input.entry, &options)?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Incomplete,
        "no unknown-struct success"
    );
    let failures = built.summary().failures();
    assert_eq!(failures.len(), 1, "one actual unknown source failure");
    let failure = failures
        .first()
        .ok_or("one actual unknown source failure")?;
    assert!(
        failure.reason().contains(
            "missionFlow condition holds the unknown struct MissionFlowCondition_Mystery"
        ),
        "actual unknown type/field: {}",
        failure.reason()
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    wire::unknown(&catalog, count)?;
    let blocking = built
        .summary()
        .export_health()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.kind == Some(p4kx_export::Kind::ContractTemplates)
                && diagnostic.stage == p4kx_export::ExportDiagnosticStage::DerivedProjection
                && diagnostic.code == p4kx_export::ExportDiagnosticCode::UnsupportedValue
                && diagnostic.severity == p4kx_export::ExportDiagnosticSeverity::Error
        })
        .count();
    assert_eq!(
        blocking, 1,
        "fully retained D owner still has one failed required C interpretation"
    );
    Ok(())
}

#[test]
fn mixed_original_and_or_not_preserve_order_null_and_empty_without_deduplication() -> TestResult {
    let input = fixture::mixed()?;
    source::mixed(&input)?;
    let built = p4kx_export::build_catalog_from(
        &input.archive,
        &input.entry,
        &CatalogOptions::new(UNIX_EPOCH),
    )?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Complete,
        "all mixed original values are readable"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    wire::mixed(&catalog)?;
    Ok(())
}
