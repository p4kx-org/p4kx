// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::{self as graph, Template};
use crate::template_flow_depth_fixture::{self as fixture, Branch, Tail};

pub(super) fn output(catalog: &Value, branch: Branch, count: usize, tail: Tail) -> TestResult {
    let graph = Template::new(catalog, fixture::GUID)?;
    let mut node = graph.condition()?;
    for _ in 0..count {
        node = step(&graph, node, branch)?;
    }
    leaf(&graph, node, tail)
}

fn step<'catalog>(
    graph: &Template<'catalog>,
    node: &Value,
    branch: Branch,
) -> TestResult<&'catalog Value> {
    let meaning = graph::condition(node)?;
    assert_eq!(
        meaning.get("type"),
        Some(&json!(branch.wire_type())),
        "actual original condition occurrence"
    );
    let reference = match branch {
        Branch::Not => meaning.get("child").ok_or("actual scalar child")?,
        Branch::And | Branch::Or => {
            let children = raw::array(meaning, "children")?;
            assert_eq!(
                children.len(),
                1,
                "every original one-child array remains complete"
            );
            children.first().ok_or("actual original child occurrence")?
        }
    };
    graph.owner.follow(reference)
}

fn leaf(graph: &Template<'_>, node: &Value, tail: Tail) -> TestResult {
    let meaning = graph::condition(node)?;
    match tail {
        Tail::Known => assert_eq!(
            meaning,
            &json!({"type":"onMissionStart"}),
            "actual known leaf"
        ),
        Tail::Missing => {
            assert_eq!(
                meaning,
                &json!({
                    "type":"not","conditionField":{"type":"field","fieldIndex":"0"},"child":null,
                }),
                "original null is distinct from an unknown condition"
            );
            raw::source_null(graph.field(node, "condition")?);
        }
        Tail::Empty => {
            assert_eq!(
                meaning,
                &json!({
                    "type":"and","conditionsField":{"type":"field","fieldIndex":"0"},"children":[],
                }),
                "empty is a real complete source array"
            );
            assert!(
                raw::elements(graph.field(node, "conditions")?)?.is_empty(),
                "the same actual raw source array is genuinely empty"
            );
        }
        Tail::Unknown => return Err("strict unknown cannot reach a complete output oracle".into()),
    }
    Ok(())
}

pub(super) fn unknown(catalog: &Value, count: usize) -> TestResult {
    let graph = Template::new(catalog, fixture::GUID)?;
    let mut node = graph.condition()?;
    for _ in 0..count {
        node = step(&graph, node, Branch::Not)?;
    }
    assert_eq!(
        graph::condition(node)?,
        &json!({"type":"unsupported"}),
        "readable original unknown is retained with failed required interpretation"
    );
    assert_eq!(
        raw::payload(node, "struct")?.pointer("/sourceType/value"),
        Some(&json!("MissionFlowCondition_Mystery")),
        "the exact readable unknown Struct remains"
    );
    Ok(())
}

pub(super) fn mixed(catalog: &Value) -> TestResult {
    let graph = Template::new(catalog, fixture::GUID)?;
    let root = graph.condition()?;
    let outer = children(root, "and", 2)?;
    let first = graph
        .owner
        .follow(outer.first().ok_or("first original AND child")?)?;
    leaf(&graph, first, Tail::Known)?;
    let alternative = graph
        .owner
        .follow(outer.get(1).ok_or("second original AND child")?)?;
    let inner = children(alternative, "or", 2)?;
    let missing = graph
        .owner
        .follow(inner.first().ok_or("first original OR child")?)?;
    leaf(&graph, missing, Tail::Missing)?;
    let empty = graph
        .owner
        .follow(inner.get(1).ok_or("second original OR child")?)?;
    leaf(&graph, empty, Tail::Empty)?;
    assert_ne!(
        outer.first(),
        outer.get(1),
        "distinct actual child occurrence links"
    );
    assert_ne!(
        inner.first(),
        inner.get(1),
        "null and empty remain distinct source occurrences"
    );
    Ok(())
}

fn children<'node>(node: &'node Value, kind: &str, count: usize) -> TestResult<&'node [Value]> {
    let meaning = graph::condition(node)?;
    assert_eq!(
        meaning.get("type"),
        Some(&json!(kind)),
        "exact original mixed operator"
    );
    let children = raw::array(meaning, "children")?;
    assert_eq!(
        children.len(),
        count,
        "exact original mixed child cardinality"
    );
    Ok(children)
}
