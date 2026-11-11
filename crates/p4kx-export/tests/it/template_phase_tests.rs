// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, Compression, ExportHealthStatus};
use serde_json::{Value, json};

use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Template;
use crate::template_phase_fixture as fixture;
use crate::template_phase_source::{self as source, Observed};

#[test]
fn thirty_three_phases_and_later_root_remain_complete() -> TestResult {
    complete(33)
}

#[test]
fn thirty_four_readable_phases_are_not_an_exporter_depth_failure() -> TestResult {
    complete(34)
}

#[test]
fn thirty_five_readable_phases_are_not_an_exporter_depth_failure() -> TestResult {
    complete(35)
}

#[test]
fn sixty_four_source_phases_are_flat_and_json_readable() -> TestResult {
    complete(64)
}

fn complete(requested: usize) -> TestResult {
    for mode in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let input = fixture::open(requested, mode)?;
        let observed = source::check(&input, requested)?;

        let options = CatalogOptions::new(UNIX_EPOCH);
        let built = p4kx_export::build_catalog_from(&input.archive, &input.entry, &options)?;
        assert_eq!(
            built.summary().export_health().status,
            ExportHealthStatus::Complete,
            "every readable phase survives without a second exporter cutoff"
        );
        assert!(
            built.summary().failures().is_empty(),
            "no selected projection failed"
        );
        output(&serde_json::from_slice(built.json())?, &observed)?;
        writers(&input, &options, built.json(), &observed)?;
    }
    Ok(())
}

#[test]
fn sixty_five_source_phases_and_later_sibling_remain_normal_complete() -> TestResult {
    complete(65)
}
fn writers(
    input: &Fixture,
    options: &CatalogOptions<'_>,
    baseline: &[u8],
    observed: &Observed,
) -> TestResult {
    for compression in [Compression::None, Compression::ZSTD] {
        let (_, bytes) = p4kx_export::write_catalog_from(
            &input.archive,
            &input.entry,
            options,
            compression,
            || Ok(Vec::new()),
        )?;
        let plain = match compression {
            Compression::None => bytes,
            Compression::Zstd { .. } => zstd::stream::decode_all(bytes.as_slice())?,
        };
        assert_eq!(
            plain, baseline,
            "complete writer output equals the source-bound build"
        );
        output(&serde_json::from_slice(&plain)?, observed)?;
    }
    Ok(())
}

fn output(catalog: &Value, observed: &Observed) -> TestResult {
    let graph = Template::new(catalog, fixture::TEMPLATE_TEXT)?;
    let objectives = raw::array(graph.projection, "objectives")?;
    assert_eq!(
        objectives.len(),
        observed.objectives.len(),
        "all original readable occurrences and the later sibling survive"
    );
    for (actual, expected) in objectives.iter().zip(&observed.objectives) {
        let id = graph.bound(
            actual
                .get("id")
                .ok_or("actual selected original id field")?,
        )?;
        assert_eq!(
            raw::payload(id, "guid")?.get("value"),
            Some(&json!(expected.id)),
            "actual source ID in preorder"
        );
        let parent = match actual
            .get("parent")
            .ok_or("required nullable physical parent")?
        {
            Value::Null => None,
            reference => {
                let parent = graph.owner.follow(reference)?;
                Some(
                    raw::payload(graph.field(parent, "id")?, "guid")?
                        .get("value")
                        .and_then(Value::as_str)
                        .ok_or("actual original parent id")?,
                )
            }
        };
        assert_eq!(
            parent,
            expected.parent.as_deref(),
            "actual immediate phase parent"
        );
        let node = graph
            .owner
            .follow(actual.get("source").ok_or("actual physical objective")?)?;
        let name = graph.field(node, "debugName")?;
        assert_eq!(
            raw::payload(name, "string")?.pointer("/value/sourceText"),
            Some(&json!(expected.name)),
            "actual source name"
        );
    }
    Ok(())
}
