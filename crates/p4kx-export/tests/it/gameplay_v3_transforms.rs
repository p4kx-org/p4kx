// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support::{self as support, TestResult};
use crate::gameplay_v3_values as values;
use p4kx_testkit::{DataForgeBuilder, TestValue};
use serde_json::{Value, json};

#[test]
fn gameplay_v3_retains_all_seven_transformation_source_types_without_evaluating_operations()
-> TestResult {
    let mut database = schema::schema();
    for (index, &(name, _)) in schema::TRANSFORMS.iter().enumerate() {
        database = database.record(values::record(
            values::FIRST + u128::try_from(index)?,
            &format!("Operation{index}"),
            operation(name),
        ));
    }
    source::declarations(&database)?;
    original_operations(&database)?;
    let (_, catalog) = support::normal(&database)?;
    assert_operations(&catalog)?;
    Ok(())
}

fn operation(name: &str) -> TestValue {
    match name {
        schema::SCALE => values::scale(TestValue::Single(1.25)),
        schema::SEQUENCE => values::sequence(Vec::new()),
        _ => values::owned(name),
    }
}

fn original_operations(database: &DataForgeBuilder) -> TestResult {
    for (index, &(expected, _)) in schema::TRANSFORMS.iter().enumerate() {
        source::with_tree(database, u32::try_from(index)?, |tree| {
            let display = source::structure(tree.root().field("displayTransformation"))?;
            assert_eq!(
                display.type_name(),
                expected,
                "the original pointer selects this concrete source"
            );
            assert_eq!(
                display.fields().len(),
                usize::from(matches!(expected, schema::SCALE | schema::SEQUENCE)),
                "the original bases and conversions are actually fieldless"
            );
            Ok(())
        })?;
    }
    Ok(())
}

fn assert_operations(catalog: &Value) -> TestResult {
    assert_eq!(
        support::rows(catalog)?.len(),
        7,
        "every source operation has its actual owner"
    );
    for (index, &(name, tag)) in schema::TRANSFORMS.iter().enumerate() {
        let row = support::row(catalog, u64::try_from(index)?)?;
        let display = operation_node(row)?;
        assert_eq!(
            support::at(display, "/sourceType/value")?,
            &json!(name),
            "fieldless concrete operations and both bases retain their actual source type"
        );
        assert_eq!(
            support::at(display, "/payload/value/type")?,
            &json!(tag),
            "the exporter control distinguishes every known operation shape"
        );
        let payload = support::at(display, "/payload/value")?
            .as_object()
            .ok_or("operation payload")?;
        let expected = if matches!(name, schema::SCALE | schema::SEQUENCE) {
            2
        } else {
            1
        };
        assert_eq!(
            payload.len(),
            expected,
            "no guessed output/base/unit or parameter is synthesized"
        );
        assert!(
            row.get("computedValue").is_none(),
            "the source declaration is not evaluated"
        );
    }
    Ok(())
}

fn operation_node(row: &Value) -> TestResult<&Value> {
    let graph = support::at(row, "/displayTransformation/value")?;
    assert_eq!(
        support::at(graph, "/root/nodeIndex")?,
        &json!(0),
        "the root is a local structural position"
    );
    assert_eq!(
        support::at(graph, "/nodes")?.as_array().map(Vec::len),
        Some(1),
        "each original single operation has one occurrence"
    );
    assert!(
        graph.get("state").is_none(),
        "the arena grouping has no fabricated source state"
    );
    let display = support::at(graph, "/nodes/0")?;
    assert!(
        display.get("state").is_none(),
        "node grouping does not duplicate the original pointer carrier"
    );
    Ok(display)
}
