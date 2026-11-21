// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind};
use serde_json::Value;

use crate::loot_extra_v3_assertions as assertions;
use crate::loot_extra_v3_fixture::{self as fixture, Case, Checked};
use crate::loot_extra_v3_graph::{self as graph, Graph};
use crate::loot_extra_v3_source as source;

#[test]
fn loot_v3_retains_every_extra_root_nested_and_array_fact() -> Checked {
    let input = fixture::archive(Case::Readable)?;
    let original = source::check(&input.archive, Case::Readable)?;
    let built = p4kx_export::build_catalog(&input.archive, &options())?;
    assert_eq!(
        built
            .summary()
            .kind(Kind::LootPoolFilters)
            .map(|kind| kind.count),
        Some(1),
        "existing physical Kind accounting"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let graph = Graph::new(&catalog, original.count())?;
    let fields = graph.fields()?;
    assert_eq!(
        fields.len(),
        5,
        "every original root declaration remains ordered"
    );
    let filter = graph.field(fields, 0, "filter", "strongPointer", false)?;
    assertions::check_extra_fields(&graph, graph::payload(filter, "struct")?)?;
    assertions::check_root_scalars(
        &graph,
        fields,
        &original,
        built.summary().source_inventory(),
    )?;
    assertions::check_slots(&graph, filter, false, original.filters_target)?;
    Ok(())
}

#[test]
fn loot_v3_preserves_null_and_error_occurrences_between_readable_slots() -> Checked {
    let input = fixture::archive(Case::Holes)?;
    let original = source::check(&input.archive, Case::Holes)?;
    let built = p4kx_export::build_catalog(&input.archive, &options())?;
    assert_eq!(
        built
            .summary()
            .kind(Kind::LootPoolFilters)
            .map(|kind| kind.count),
        Some(1),
        "an unreadable child does not drop its original physical row"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let graph = Graph::new(&catalog, original.count())?;
    let root = graph.fields()?;
    assert_eq!(
        root.len(),
        5,
        "retained error does not erase the other root fields"
    );
    let filter = graph.field(root, 0, "filter", "strongPointer", false)?;
    let slots = assertions::check_slots(&graph, filter, true, original.filters_target)?;
    assertions::check_holes(&graph, slots, &original, built.summary().source_inventory())?;
    assertions::check_root_scalars(&graph, root, &original, built.summary().source_inventory())?;
    assertions::check_extra_fields(&graph, graph::payload(filter, "struct")?)?;
    Ok(())
}

fn options() -> CatalogOptions<'static> {
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    options
}
