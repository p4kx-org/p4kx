// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_extraction::Value as Raw;
use p4kx_testkit::{DataForgeBuilder, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::gameplay_v3_consumer as consumer;
use crate::gameplay_v3_consumer::gameplay_properties::transformations::TransformationError;
use crate::gameplay_v3_original as original;
use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support as support;
use crate::gameplay_v3_values as values;

#[derive(Debug, Clone, Copy)]
enum Failure {
    Shape,
    Root,
    Empty,
    Range,
    Cycle,
    Ownership,
    Unreachable,
    Order,
}

pub(crate) fn controls(catalog: &Value) -> TestResult {
    let first = format!("{}/displayTransformation/value", support::FIRST);
    reference_controls(catalog, &first)?;
    orphan_copy(catalog, &first)?;
    reordered(catalog, &first)
}

fn reference_controls(catalog: &Value, first: &str) -> TestResult {
    for (relative, replacement, expected) in [
        ("/root/nodeIndex", json!("0"), Failure::Shape),
        ("/root/nodeIndex", json!(1), Failure::Root),
        (
            "/nodes/0/payload/value/transformations/value/0/value/nodeIndex",
            json!(u32::MAX),
            Failure::Range,
        ),
        (
            "/nodes/0/payload/value/transformations/value/0/value/nodeIndex",
            json!(0),
            Failure::Cycle,
        ),
        (
            "/nodes/0/payload/value/transformations/value/2/value/nodeIndex",
            json!(1),
            Failure::Ownership,
        ),
    ] {
        let mut changed = catalog.clone();
        *changed
            .pointer_mut(&format!("{first}{relative}"))
            .ok_or("actual local control")? = replacement;
        rejected(&changed, expected)?;
    }
    let mut empty = catalog.clone();
    *empty
        .pointer_mut(&format!("{first}/nodes"))
        .ok_or("actual occurrence vector")? = json!([]);
    rejected(&empty, Failure::Empty)?;
    Ok(())
}

fn orphan_copy(catalog: &Value, first: &str) -> TestResult {
    let mut copy = catalog.clone();
    let node = support::at(catalog, &format!("{first}/nodes/1"))?.clone();
    copy.pointer_mut(&format!("{first}/nodes"))
        .and_then(Value::as_array_mut)
        .ok_or("actual arena")?
        .push(node);
    rejected(&copy, Failure::Unreachable)?;
    Ok(())
}

fn reordered(catalog: &Value, first: &str) -> TestResult {
    let mut reordered = catalog.clone();
    for (slot, target) in [(0, 2), (2, 1)] {
        *reordered
            .pointer_mut(&format!(
                "{first}/nodes/0/payload/value/transformations/value/{slot}/value/nodeIndex"
            ))
            .ok_or("actual ordered child")? = json!(target);
    }
    rejected(&reordered, Failure::Order)
}

fn rejected(catalog: &Value, expected: Failure) -> TestResult {
    let Err(error) = consumer::rows(catalog, Kind::GameplayProperties) else {
        return Err(format!("the actual canonical consumer accepted {expected:?}").into());
    };
    let actual = error
        .downcast_ref::<TransformationError>()
        .ok_or("typed actual graph failure")?;
    let matching = matches!(
        (expected, actual),
        (Failure::Shape, TransformationError::Shape(_))
            | (Failure::Root, TransformationError::Root)
            | (Failure::Empty, TransformationError::Empty)
            | (Failure::Range, TransformationError::Range)
            | (Failure::Cycle, TransformationError::Cycle)
            | (Failure::Ownership, TransformationError::Ownership)
            | (Failure::Unreachable, TransformationError::Unreachable)
            | (Failure::Order, TransformationError::Order)
    );
    assert!(
        matching,
        "the actual {expected:?} guard fails before an unrelated ordering/shape check: {actual:?}"
    );
    Ok(())
}

pub(crate) fn equal_original_nodes() -> TestResult {
    let database = equal_database()?;
    let (_, catalog) = support::normal(&database)?;
    original::check(&catalog, &database, 0)?;
    assert_equal_links(&catalog)?;
    equal_groups(&catalog, &database)
}

fn equal_database() -> TestResult<DataForgeBuilder> {
    let database = schema::schema().record(values::record(
        values::FIRST,
        "Equal",
        values::sequence(vec![
            values::scale(TestValue::Single(1.25)),
            values::scale(TestValue::Single(1.25)),
        ]),
    ));
    source::with_tree(&database, 0, |tree| {
        let display = source::structure(tree.root().field("displayTransformation"))?;
        let slots = source::array(display.field("transformations"))?;
        assert_eq!(
            slots.len(),
            2,
            "two original independently owned occurrences"
        );
        for slot in slots {
            let scale = source::structure(Some(slot))?;
            assert_eq!(
                scale.field("scale"),
                Some(&Raw::Single(1.25)),
                "the original payloads are equal"
            );
        }
        assert_eq!(
            tree.error_count(),
            0,
            "equal values did not become OwnedTwice"
        );
        Ok(())
    })?;
    Ok(database)
}

fn assert_equal_links(catalog: &Value) -> TestResult {
    let graph = support::at(support::row(catalog, 0)?, "/displayTransformation/value")?;
    assert_eq!(
        support::at(graph, "/nodes")?.as_array().map(Vec::len),
        Some(3),
        "no deep-equality deduplication"
    );
    for (slot, index) in [(0, 1), (1, 2)] {
        assert_eq!(
            support::at(
                graph,
                &format!("/nodes/0/payload/value/transformations/value/{slot}/value/nodeIndex")
            )?,
            &json!(index),
            "equal original payloads keep distinct local links"
        );
    }
    Ok(())
}

fn equal_groups(catalog: &Value, database: &DataForgeBuilder) -> TestResult {
    let graph = support::at(support::row(catalog, 0)?, "/displayTransformation/value")?;
    let mut equal_groups = catalog.clone();
    let node = support::at(graph, "/nodes/1")?.clone();
    *equal_groups
        .pointer_mut(&format!(
            "{}/displayTransformation/value/nodes/2",
            support::FIRST
        ))
        .ok_or("second actual occurrence")? = node;
    consumer::rows(&equal_groups, Kind::GameplayProperties)?;
    assert!(
        original::check(&equal_groups, database, 0).is_err(),
        "only the original oracle rejects the copied source path; equal groups are structurally valid"
    );
    Ok(())
}

pub(crate) fn foreign_source_node() -> TestResult {
    let database = values::simple().record(values::record(
        values::SECOND,
        "OtherOwner",
        values::scale(TestValue::Single(1.25)),
    ));
    for index in [0, 1] {
        source::with_tree(&database, index, |tree| {
            assert_eq!(
                tree.record_index(),
                index,
                "each equal original payload has its actual physical owner"
            );
            assert_eq!(
                tree.type_name(),
                schema::PROPERTY,
                "both original owners are the selected source family"
            );
            let scale = source::structure(tree.root().field("displayTransformation"))?;
            assert_eq!(
                scale.type_name(),
                schema::SCALE,
                "both original concrete nodes have the same declared source type"
            );
            assert_eq!(
                scale.field("scale"),
                Some(&Raw::Single(1.25)),
                "equal bits cannot expose the foreign source by value"
            );
            Ok(())
        })?;
    }
    let (_, catalog) = support::normal(&database)?;
    for index in [0, 1] {
        original::check(&catalog, &database, index)?;
    }
    let node = support::at(
        support::row(&catalog, 1)?,
        "/displayTransformation/value/nodes/0",
    )?
    .clone();
    let mut foreign = catalog;
    *foreign
        .pointer_mut(&format!(
            "{}/displayTransformation/value/nodes/0",
            support::FIRST
        ))
        .ok_or("actual first owner node")? = node;
    consumer::rows(&foreign, Kind::GameplayProperties)?;
    let error = original::check(&foreign, &database, 0)
        .err()
        .ok_or("foreign original owner must fail")?;
    assert!(
        error.to_string().contains("original contribution differs"),
        "the independent original-owner/SourceUse oracle, not structural indices or equal values, rejects the copy: {error}"
    );
    Ok(())
}
