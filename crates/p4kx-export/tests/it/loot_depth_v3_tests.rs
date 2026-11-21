// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogError, CatalogOptions, Compression, ExportHealthStatus};
use p4kx_extraction::CancelToken;
use serde_json::{Value, json};

use crate::health_origin_fixture::{Fixture, TestResult};
use crate::loot_depth_v3_fixture as fixture;
use crate::loot_depth_v3_source::{self as source, Materialized};
use crate::loot_v3_graph::{self as graph, Graph};

#[test]
fn full_canonical_owned_depth_roundtrips_whole_default_json_and_zstd() -> TestResult {
    for requested in [0, 64] {
        let input = fixture::open(requested)?;
        let observed = source::check(&input, requested)?;
        let options = CatalogOptions::new(UNIX_EPOCH);
        let baseline = p4kx_export::build_catalog_from(&input.archive, &input.entry, &options)?;
        assert_eq!(
            baseline.summary().export_health().status,
            ExportHealthStatus::Complete,
            "every original source node remains complete"
        );
        check_output(&serde_json::from_slice(baseline.json())?, &observed)?;
        check_writers(&input, &options, baseline.json(), &observed)?;
    }
    Ok(())
}

#[test]
fn sixty_five_original_owned_nodes_remain_normal_complete_and_roundtrip() -> TestResult {
    let input = fixture::open(65)?;
    let observed = source::check(&input, 65)?;
    let options = CatalogOptions::new(UNIX_EPOCH);
    let baseline = p4kx_export::build_catalog_from(&input.archive, &input.entry, &options)?;
    assert_eq!(
        baseline.summary().export_health().status,
        ExportHealthStatus::Complete,
        "all original nodes are decoded before publication"
    );
    check_output(&serde_json::from_slice(baseline.json())?, &observed)?;
    check_writers(&input, &options, baseline.json(), &observed)
}
#[test]
fn source_depth_writer_cancellation_stays_explicit_before_and_after_creation() -> TestResult {
    let input = fixture::open(0)?;
    source::check(&input, 0)?;
    for compression in [Compression::None, Compression::ZSTD] {
        cancelled(&input, compression, false);
        cancelled(&input, compression, true);
    }
    Ok(())
}

fn cancelled(input: &Fixture, compression: Compression, at_creation: bool) {
    let cancel = CancelToken::new();
    if !at_creation {
        cancel.cancel();
    }
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.cancel = Some(&cancel);
    let creates = Cell::new(0);
    let result = p4kx_export::write_catalog_from(
        &input.archive,
        &input.entry,
        &options,
        compression,
        || {
            creates.set(creates.get() + 1);
            cancel.cancel();
            Ok(Vec::new())
        },
    );
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "cancellation never returns successful partial JSON"
    );
    assert_eq!(
        creates.get(),
        usize::from(at_creation),
        "creation is reached only for cancellation requested by that callback"
    );
}

fn check_writers(
    input: &Fixture,
    options: &CatalogOptions<'_>,
    baseline: &[u8],
    observed: &Materialized,
) -> TestResult {
    for compression in [Compression::None, Compression::ZSTD] {
        let creates = Cell::new(0);
        let (summary, bytes) = p4kx_export::write_catalog_from(
            &input.archive,
            &input.entry,
            options,
            compression,
            || {
                creates.set(creates.get() + 1);
                Ok(Vec::new())
            },
        )?;
        assert_eq!(
            creates.get(),
            1,
            "one actual destination is created after publication policy"
        );
        assert_eq!(
            summary.export_health().status,
            ExportHealthStatus::Complete,
            "writer sizing/compression does not replace the actual source health"
        );
        let unpacked = match compression {
            Compression::None => bytes,
            Compression::Zstd { .. } => zstd::stream::decode_all(bytes.as_slice())?,
        };
        assert_eq!(
            unpacked, baseline,
            "whole writer passes preserve the same frozen source and complete materialized graph"
        );
        let catalog: Value = serde_json::from_slice(&unpacked)?;
        check_output(&catalog, observed)?;
    }
    Ok(())
}

fn check_output(catalog: &Value, observed: &Materialized) -> TestResult {
    let owner = Graph::new(catalog, fixture::GUID)?;
    assert_eq!(
        catalog
            .pointer("/definitions/lootPoolFilters/rows/0/values")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(observed.values),
        "every original materialized Struct/scalar/null/error slot survives without another exporter depth cut"
    );
    let mut node = owner.root("filter")?;
    for ordinal in 0..observed.nodes {
        let fields = graph::structure(node, "DepthNode")?;
        assert_eq!(fields.len(), 2, "both actual nested fields remain ordered");
        let marker = owner.field(fields, "marker")?;
        let number = graph::payload(marker, "number")?
            .get("value")
            .ok_or("original exact UInt32")?;
        assert_eq!(
            number.get("type"),
            Some(&json!("uint32")),
            "original numeric storage width"
        );
        assert_eq!(
            number.get("value"),
            Some(&json!(u32::try_from(ordinal)?)),
            "all original marker values survive"
        );
        node = owner.field(fields, "next")?;
    }
    graph::source_null(node);
    Ok(())
}
