// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{
    BuiltCatalog, CatalogError, CatalogOptions, CatalogPublicationMode, Compression,
    ExportDiagnosticCode, ExportDiagnosticStage, ExportHealthStatus, Kind,
};
use serde_json::Value;

use crate::broker_flow_fixture as fixture;
use crate::broker_flow_graph::Broker;
use crate::broker_flow_source as source;
use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::template_flow_depth_fixture::{Branch, Tail};

const DEEP_WIRE_EDGES: usize = 257;

#[test]
fn seventeen_and_eighteen_readable_broker_not_edges_are_complete() -> TestResult {
    complete(Branch::Not, 17, Tail::Known)?;
    complete(Branch::Not, 18, Tail::Known)
}

#[test]
fn broker_and_chain_crosses_default_recursive_wire_depth_without_a_quota() -> TestResult {
    complete(Branch::And, DEEP_WIRE_EDGES, Tail::Known)
}

#[test]
fn broker_or_chain_crosses_default_recursive_wire_depth_without_a_quota() -> TestResult {
    complete(Branch::Or, DEEP_WIRE_EDGES, Tail::Known)
}

#[test]
fn broker_genuine_null_and_empty_children_remain_distinct() -> TestResult {
    complete(Branch::Not, 0, Tail::Missing)?;
    complete(Branch::And, 0, Tail::Empty)
}

fn complete(branch: Branch, count: usize, tail: Tail) -> TestResult {
    let mut inputs = Vec::new();
    for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let input = fixture::open(branch, count, tail, kind)?;
        source::check(&input, branch, count, tail)?;
        inputs.push((input, kind));
    }
    for (input, kind) in inputs {
        let options = crate::health_origin_fixture::options();
        let built = p4kx_export::build_catalog_from(&input.archive, &input.entry, &options)?;
        assert_eq!(
            built.summary().export_health().status,
            ExportHealthStatus::Complete,
            "every actual original value is readable and its required interpretation is supported"
        );
        let document: Value = serde_json::from_slice(built.json())?;
        let graph = Broker::new(&document)?;
        graph.chain(branch, count, tail)?;
        graph.projected(&document)?;
        assert_eq!(
            crate::broker_flow_graph::projections(&document)?.len(),
            1,
            "one exact original Broker projection without fabricated rows"
        );
        assert_eq!(
            graph.owner.values.len(),
            original_count(branch, count, tail),
            "each complete original occurrence appears exactly once"
        );
        crate::broker_flow_origins::check(
            &document,
            built.summary().source_inventory(),
            kind,
            original_count(branch, count, tail),
        )?;
        crate::health_origin_assertions::closure(&document)?;
        writers(&input, &options, built.json())?;
    }
    Ok(())
}

fn original_count(branch: Branch, count: usize, tail: Tail) -> usize {
    let width = if matches!(branch, Branch::Not) { 1 } else { 2 };
    4 + count * width + usize::from(matches!(tail, Tail::Missing | Tail::Empty))
}

fn writers(input: &Fixture, options: &CatalogOptions<'_>, expected: &[u8]) -> TestResult {
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
            "actual full writer health"
        );
        let plain = match compression {
            Compression::None => bytes,
            Compression::Zstd { .. } => zstd::stream::decode_all(bytes.as_slice())?,
        };
        assert_eq!(
            plain, expected,
            "the complete actual writer document is identical"
        );
        let _: Value = serde_json::from_slice(&plain)?;
    }
    Ok(())
}

#[test]
fn late_readable_broker_unknown_keeps_raw_and_failed_required_interpretation() -> TestResult {
    let mut inputs = Vec::new();
    for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let input = fixture::open(Branch::Not, 18, Tail::Unknown, kind)?;
        source::check(&input, Branch::Not, 18, Tail::Unknown)?;
        inputs.push((input, kind));
    }
    for (input, kind) in inputs {
        let mut options = crate::health_origin_fixture::options();
        refusal(&input);

        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let built = p4kx_export::build_catalog_from(&input.archive, &input.entry, &options)?;
        assert_eq!(
            built.summary().export_health().status,
            ExportHealthStatus::Incomplete,
            "readable raw never becomes successful required interpretation"
        );
        let document: Value = serde_json::from_slice(built.json())?;
        let graph = Broker::new(&document)?;
        graph.chain(Branch::Not, 18, Tail::Unknown)?;
        assert_primary_unknown(&built)?;
        crate::broker_flow_origins::unsupported(&built, kind, 21)?;
        let failures = built
            .summary()
            .export_health()
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.kind == Some(Kind::MissionBrokerEntries)
                    && diagnostic.stage == ExportDiagnosticStage::DerivedProjection
                    && diagnostic.code == ExportDiagnosticCode::UnsupportedValue
            })
            .count();
        assert_eq!(
            failures, 1,
            "one failed required C interpretation with the emitted D owner retained"
        );
        assert!(
            crate::broker_flow_graph::projections(&document)?.is_empty(),
            "no successful partial Broker projection"
        );
        crate::broker_flow_origins::check(
            &document,
            built.summary().source_inventory(),
            kind,
            original_count(Branch::Not, 18, Tail::Unknown),
        )?;
        crate::health_origin_assertions::closure(&document)?;
    }
    Ok(())
}

fn assert_primary_unknown(built: &BuiltCatalog) -> TestResult {
    let failures = built.summary().failures();
    assert_eq!(
        failures.len(),
        1,
        "the actual primary unsupported cause remains unique"
    );
    assert!(
        failures
            .first()
            .ok_or("actual failed required interpretation")?
            .reason()
            .contains(
                "missionFlow condition holds the unknown struct MissionFlowCondition_Mystery"
            ),
        "the late actual unsupported type is not an artificial depth failure"
    );
    Ok(())
}

pub(crate) fn refusal(input: &Fixture) {
    let options = crate::health_origin_fixture::options();
    assert!(
        matches!(
            p4kx_export::build_catalog_from(&input.archive, &input.entry, &options),
            Err(CatalogError::IncompleteExport(_))
        ),
        "the same unsupported required meaning refuses Normal"
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
            "each complete writer refuses"
        );
        assert!(
            !created,
            "no complete-looking prefix or destination is created"
        );
    }
}
