// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, Kind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use crate::export_complete_loot_fixture as fixture;
use crate::export_complete_loot_source;
use crate::loot_v3_graph::{self as graph, Graph};
use crate::starmap_fixture;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn readable_filter_children_past_sixteen_levels_never_become_an_empty_authored_list() -> TestResult
{
    for layers in [0, 3, 17] {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("Data.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::zstd(
                "Data\\Game2.dcb",
                fixture::database(layers),
            ))
            .write_to(&path)?;
        let archive = Archive::open(&path)?;
        export_complete_loot_source::check(&archive, layers)?;
        let options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
        let built = build_catalog(&archive, &options)?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        assert_eq!(
            catalog.get("schemaVersion"),
            Some(&json!(3)),
            "only the current catalog schema"
        );
        assert_eq!(
            built
                .summary()
                .kind(Kind::LootPoolFilters)
                .map(|kind| kind.count),
            Some(3),
            "the original chain, target and empty sequence remain independent rows"
        );
        assert_chain(&catalog, layers)?;
        assert_absence(&catalog)?;
    }
    Ok(())
}

fn assert_chain(catalog: &Value, layers: usize) -> TestResult {
    let graph = Graph::new(catalog, &starmap_fixture::guid_text(fixture::CHAIN))?;
    let mut node = graph.root("filter")?;
    for ordinal in 0..layers {
        let fields = graph::structure(node, "PoolFilter_Sequence")?;
        assert_eq!(
            fields.len(),
            1,
            "sequence {ordinal} has its actual original filters field"
        );
        let slots = graph::elements(graph.field(fields, "filters")?)?;
        assert_eq!(
            slots.len(),
            1,
            "original sequence {ordinal} retains its actual child"
        );
        let child = graph.follow(slots.first().ok_or("projected ordered child")?)?;
        let fields = graph::structure(child, "PoolFilterInstance")?;
        assert_eq!(
            fields.len(),
            3,
            "every original instance declaration is retained"
        );
        assert_eq!(
            graph::public_text(graph.field(fields, "name")?)?,
            format!("step-{ordinal}"),
            "original label order"
        );
        assert_eq!(
            graph::public_text(graph.field(fields, "mode")?)?,
            "Cumulative",
            "original mode"
        );
        node = graph.field(fields, "filter")?;
    }
    let fields = graph::structure(node, "PoolFilter_RecordRef")?;
    assert_eq!(
        fields.len(),
        1,
        "the original terminal node has its authored Reference field"
    );
    assert_eq!(
        graph::reference_guid(graph.field(fields, "filterRecord")?)?,
        starmap_fixture::guid_text(fixture::TARGET),
        "the original terminal target survives every readable level"
    );
    Ok(())
}

fn assert_absence(catalog: &Value) -> TestResult {
    let target = Graph::new(catalog, &starmap_fixture::guid_text(fixture::TARGET))?;
    graph::source_null(target.root("filter")?);
    let empty = Graph::new(catalog, &starmap_fixture::guid_text(fixture::EMPTY))?;
    let fields = graph::structure(empty.root("filter")?, "PoolFilter_Sequence")?;
    let filters = empty.field(fields, "filters")?;
    assert_eq!(
        graph::payload(filters, "array")?.get("elements"),
        Some(&json!([])),
        "a genuinely empty authored list stays empty"
    );
    Ok(())
}
