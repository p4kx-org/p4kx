// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::gameplay_v3_deep as deep;
use crate::gameplay_v3_graph_cases as graph_cases;
use crate::gameplay_v3_original as original;
use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support::{self as support, TestResult};
use crate::gameplay_v3_values as values;
use p4kx_extraction::Value as Raw;
use p4kx_testkit::TestValue;
use serde_json::{Value, json};

#[test]
fn gameplay_v3_recursive_sequences_preserve_original_slots_and_the_existing_depth_boundary()
-> TestResult {
    let database = schema::schema()
        .record(values::record(
            values::FIRST,
            "Primary",
            values::recursive(),
        ))
        .record(values::record(values::SECOND, "Null", TestValue::Null))
        .record(values::record(
            values::SECOND + 1,
            "Empty",
            values::sequence(Vec::new()),
        ));
    original_sequence(&database)?;
    let (_, catalog) = support::normal(&database)?;
    assert_nested(&catalog)?;
    for record in 0..3 {
        original::check(&catalog, &database, record)?;
    }
    graph_cases::controls(&catalog)?;
    graph_cases::equal_original_nodes()?;
    graph_cases::foreign_source_node()?;
    deep::boundary()
}

fn original_sequence(database: &p4kx_testkit::DataForgeBuilder) -> TestResult {
    source::with_tree(database, 0, |tree| {
        let sequence = source::structure(tree.root().field("displayTransformation"))?;
        let slots = source::array(sequence.field("transformations"))?;
        assert_eq!(slots.len(), 3, "three original sequence positions");
        assert!(
            matches!(slots.get(1), Some(Raw::Null)),
            "an actual strong-pointer slot is null"
        );
        let nested = source::structure(slots.get(2))?;
        assert_eq!(
            nested.type_name(),
            schema::SEQUENCE,
            "original recursive concrete type"
        );
        assert_eq!(
            source::array(nested.field("transformations"))?.len(),
            2,
            "nested ordered positions"
        );
        Ok(())
    })?;
    source::with_tree(database, 1, |tree| {
        assert!(
            matches!(tree.root().field("displayTransformation"), Some(Raw::Null)),
            "the scalar source pointer is genuinely null"
        );
        Ok(())
    })?;
    source::with_tree(database, 2, |tree| {
        let sequence = source::structure(tree.root().field("displayTransformation"))?;
        assert!(
            source::array(sequence.field("transformations"))?.is_empty(),
            "the original empty sequence is a declared present array"
        );
        Ok(())
    })?;
    Ok(())
}

fn assert_nested(catalog: &Value) -> TestResult {
    let outer = support::at(
        support::row(catalog, 0)?,
        &format!("{}/transformations/value", support::DISPLAY),
    )?;
    assert_eq!(
        outer.as_array().map(Vec::len),
        Some(3),
        "source slots are not concatenated or compacted"
    );
    assert_eq!(
        support::at(outer, "/1/state")?,
        &json!("null"),
        "null keeps its original ordinal"
    );
    assert_eq!(
        support::at(outer, "/0/value/nodeIndex")?,
        &json!(1),
        "first source child keeps its own occurrence"
    );
    assert_eq!(
        support::at(outer, "/2/value/nodeIndex")?,
        &json!(2),
        "nested sequence stays a separate operation"
    );
    assert_nested_graph(catalog)?;
    support::state(
        support::at(support::row(catalog, 1)?, "/displayTransformation")?,
        "null",
    )?;
    assert_eq!(
        support::at(
            support::row(catalog, 2)?,
            &format!("{}/transformations/value", support::DISPLAY)
        )?,
        &json!([]),
        "empty sequence is not null"
    );
    Ok(())
}

fn assert_nested_graph(catalog: &Value) -> TestResult {
    let graph = support::at(support::row(catalog, 0)?, "/displayTransformation/value")?;
    assert_eq!(
        support::at(graph, "/nodes")?.as_array().map(Vec::len),
        Some(5),
        "every original struct has one node"
    );
    assert_eq!(
        support::at(
            graph,
            "/nodes/2/payload/value/transformations/value/0/value/nodeIndex"
        )?,
        &json!(3),
        "the nested original first slot links to its actual leaf occurrence"
    );
    assert_eq!(
        support::at(
            graph,
            "/nodes/2/payload/value/transformations/value/1/value/nodeIndex"
        )?,
        &json!(4),
        "repeated equal operations remain distinct original occurrences"
    );
    assert_eq!(
        support::at(graph, "/nodes/3/payload/value/scale/value/rawBits")?,
        &json!("80000000"),
        "the nested original signed-zero token survives"
    );
    Ok(())
}
