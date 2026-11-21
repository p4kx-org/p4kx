// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::tree_cursor::Cursor;
use crate::{DataForge, StructValue, TreeBuilder, Value};
use std::io;

mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/it/owned_source_fixture.rs"
    ));
}

#[test]
fn retained_positions_share_actual_prefixes_and_report_logical_capacities() -> fixture::TestResult {
    let bytes = fixture::database(512, 64);
    let forge = DataForge::open(&bytes)?;
    original(&forge);
    let record = forge.record(0).ok_or("actual Holder Record")?;
    let mut builder = TreeBuilder::new();
    let tree = builder.build(&record)?;
    let (counts, position_nodes, position_capacity) = actual_costs(&builder, &tree);
    profile(&builder, &counts, position_nodes, position_capacity)
}

fn original(forge: &DataForge<'_>) {
    let survey = forge
        .survey()
        .expect("the actual source layouts can be surveyed");
    assert_eq!(
        survey.unreadable_structs, 0,
        "actual original declarations are readable"
    );
    assert_eq!(
        survey.pointers.narrow, 32_769,
        "all actual nonnull source pointers exist"
    );
    assert_eq!(
        survey.pointers.null, 512,
        "one original terminal null per branch"
    );
}

fn actual_costs(builder: &TreeBuilder<'_>, tree: &crate::RecordTree<'_>) -> (Counts, usize, usize) {
    assert_eq!(
        tree.error_count(),
        0,
        "the complete valid source is materialized"
    );
    let counts = result_counts(tree.root());
    assert_eq!(
        counts.values, 66_050,
        "the actual complete source occurrence count"
    );
    assert_eq!(
        counts.fields, 65_538,
        "original root and Node fields survive"
    );
    assert_eq!(
        counts.elements, 512,
        "every original root array slot survives"
    );
    assert_eq!(
        builder.owned.len(),
        32_769,
        "each physical instance is claimed once"
    );
    let (position_nodes, position_capacity) = builder.positions.logical_lengths();
    assert_eq!(
        position_nodes, 32_770,
        "structural prefixes share parents without ancestor copies"
    );
    assert!(
        position_capacity >= position_nodes,
        "reported actual capacity includes every edge"
    );
    assert_eq!(
        builder.weak.len(),
        1,
        "only the original final weak slot is pending"
    );
    assert_eq!(
        builder.locations.len(),
        1,
        "one demanded original instance location is cached"
    );
    assert_eq!(
        builder.peak_frames, 66,
        "actual continuation peak includes root and array frames"
    );
    (counts, position_nodes, position_capacity)
}
struct Counts {
    values: usize,
    fields: usize,
    field_capacity: usize,
    elements: usize,
}

fn result_counts(root: &StructValue<'_>) -> Counts {
    let mut counts = Counts {
        values: 0,
        fields: root.fields.len(),
        field_capacity: root.fields.capacity(),
        elements: 0,
    };
    let mut cursor = Cursor::new(&root.fields);
    while let Some(visit) = cursor.next() {
        counts.values += 1;
        match visit.value {
            Value::Struct(structure) => {
                counts.fields += structure.fields.len();
                counts.field_capacity += structure.fields.capacity();
            }
            Value::Array(items) => counts.elements += items.len(),
            _ => {}
        }
    }
    counts
}

fn profile(
    builder: &TreeBuilder<'_>,
    counts: &Counts,
    positions: usize,
    position_capacity: usize,
) -> fixture::TestResult {
    use std::io::Write as _;
    let error_stream = io::stderr();
    let mut output = error_stream.lock();
    writeln!(
        output,
        "CANONICAL_OWNED_CAPACITY values={} fields={} field_capacity={} elements={} owned={} owned_capacity={} position_nodes={} position_capacity={} weak={} weak_capacity={} locations={} location_capacity={} path_capacity={} peak_frames={}",
        counts.values,
        counts.fields,
        counts.field_capacity,
        counts.elements,
        builder.owned.len(),
        builder.owned.capacity(),
        positions,
        position_capacity,
        builder.weak.len(),
        builder.weak.capacity(),
        builder.locations.len(),
        builder.locations.capacity(),
        builder.position_scratch.capacity(),
        builder.peak_frames,
    )?;
    Ok(())
}
