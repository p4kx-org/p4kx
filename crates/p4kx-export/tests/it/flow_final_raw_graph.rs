// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceInventory, SourceUseRole};
use serde_json::{Value, json};

use crate::flow_final_declaration;
use crate::flow_final_graph::Graph;
use crate::flow_final_origin::Evidence;
use crate::flow_final_original::{Configuration, Edge, Node, SourceKind};
use crate::health_origin_fixture::{Input, TestResult};
use crate::loot_v3_graph as raw;

pub(crate) fn check(
    graph: &Graph<'_>,
    original: &Configuration,
    inventory: &SourceInventory,
    input: Input,
) -> TestResult {
    let evidence = Evidence {
        inventory,
        encrypted: input.marker(),
        input_ref: graph.input_ref,
    };
    assert_eq!(
        graph.configuration.get("declarationIndex"),
        Some(&json!(
            original.declaration_index.map(|value| value.to_string())
        )),
        "actual root declaration ordinal, or genuine absence"
    );
    assert_eq!(
        graph.values.len(),
        original.nodes.len(),
        "all and only actual original occurrences, with no ghost slots"
    );
    let cell = graph
        .configuration
        .get("declaration")
        .ok_or("required sourced declaration presence")?;
    match (&original.declaration, original.nodes.first()) {
        (None, None) => absent(cell, &evidence)?,
        (Some(declaration), Some(root)) => {
            assert_eq!(
                cell.get("state"),
                Some(&json!("present")),
                "the actual declaration is a navigation binding"
            );
            let reference = cell
                .get("field")
                .ok_or("actual declaration field reference")?;
            let field = graph.owner.field(reference)?;
            assert_eq!(
                graph.owner.position(reference)?,
                (
                    None,
                    u64::try_from(
                        original
                            .declaration_index
                            .ok_or("actual root field index")?
                    )?
                ),
                "the actual Record-root declaration position is retained"
            );
            flow_final_declaration::check(field, declaration, &evidence, root.declaration_ordinal)?;
            assert_eq!(
                field.get("value"),
                Some(&graph.owner.reference(root.ordinal)?),
                "actual original root resolves in the one physical owner arena"
            );
        }
        _ => return Err("inconsistent independent original fixture witness".into()),
    }
    for (index, node) in original.nodes.iter().enumerate() {
        occurrence(graph, original, index, node, &evidence)?;
    }
    Ok(())
}

fn absent(cell: &Value, evidence: &Evidence<'_>) -> TestResult {
    assert_eq!(
        cell.get("state"),
        Some(&json!("absent")),
        "a missing declaration is sourced absence"
    );
    assert_eq!(
        cell.get("value"),
        None,
        "no fabricated declaration or null Value occurrence"
    );
    assert_eq!(
        cell.get("sourceSetComplete"),
        Some(&json!(true)),
        "the actual declaration survey is complete"
    );
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&json!(evidence.encrypted)),
        "absence is sourced from the actual admitted declaration survey"
    );
    let uses = raw::array(cell, "sourceUsesRefs")?;
    assert!(
        !uses.is_empty(),
        "absence has actual original declaration inputs"
    );
    for id in uses {
        let actual = evidence
            .inventory
            .uses()
            .iter()
            .find(|item| Some(item.id().as_str()) == id.as_str())
            .ok_or("actual absence evidence")?;
        assert_eq!(
            actual.source().as_str(),
            evidence.input_ref,
            "absence contributors belong to the actual original input"
        );
        assert!(
            !matches!(actual.location(), p4kx_extraction::SourceLocation::Subresource { namespace, .. } if namespace == "dataforgeValue"),
            "no invented ordinal for a nonexistent Value"
        );
    }
    Ok(())
}

fn occurrence(
    graph: &Graph<'_>,
    original: &Configuration,
    index: usize,
    source: &Node,
    evidence: &Evidence<'_>,
) -> TestResult {
    let actual = graph
        .values
        .get(index)
        .ok_or("actual corresponding arena node")?;
    assert_eq!(
        actual.get("sourceOrdinal"),
        Some(&json!(source.ordinal.to_string())),
        "absolute original DFS ordinal is distinct from arena index"
    );
    assert!(
        actual.get("condition").is_some(),
        "required nullable interpretation is separate from source facts"
    );
    let cell = actual
        .get("value")
        .ok_or("one immediate sourced original datum")?;
    let unreadable = matches!(source.kind, SourceKind::InvalidPointer { .. });
    evidence.value(
        cell,
        source.ordinal,
        "$value",
        if unreadable {
            SourceUseRole::Attempted
        } else {
            SourceUseRole::Payload
        },
    )?;
    match &source.kind {
        SourceKind::Struct(name) => {
            structure(graph, original, index, actual, source, name, evidence)
        }
        SourceKind::Array => {
            let references = raw::elements(actual)?;
            edges(graph, references, original, index, Edge::Element)?;
            Ok(())
        }
        SourceKind::Null => {
            raw::source_null(actual);
            Ok(())
        }
        SourceKind::InvalidPointer {
            struct_index,
            instance_index,
        } => error(
            actual,
            source.ordinal,
            *struct_index,
            *instance_index,
            evidence,
        ),
        SourceKind::Scalar(kind) => {
            raw::payload(actual, kind)?;
            Ok(())
        }
    }
}

fn structure(
    graph: &Graph<'_>,
    original: &Configuration,
    index: usize,
    actual: &Value,
    source: &Node,
    name: &str,
    evidence: &Evidence<'_>,
) -> TestResult {
    let payload = raw::payload(actual, "struct")?;
    let source_type = payload
        .get("sourceType")
        .ok_or("independent original concrete type")?;
    assert_eq!(
        raw::present(source_type)?,
        &json!(name),
        "actual original struct type"
    );
    evidence.value(
        source_type,
        source.ordinal,
        "$type",
        SourceUseRole::Selector,
    )?;
    let fields = raw::array(payload, "fields")?;
    assert_eq!(
        fields.len(),
        source.fields.len(),
        "every original declaration including unknown/additional fields"
    );
    let children: Vec<_> = original
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.parent == Some(index))
        .collect();
    assert_eq!(
        fields.len(),
        children.len(),
        "one child occurrence per original field"
    );
    for (slot, ((field, declaration), (_child_index, child))) in
        fields.iter().zip(&source.fields).zip(children).enumerate()
    {
        assert_eq!(
            child.edge,
            Edge::Field(slot),
            "actual original field ordinal within this struct"
        );
        flow_final_declaration::check(field, declaration, evidence, child.declaration_ordinal)?;
        assert_eq!(
            field.get("value"),
            Some(&graph.owner.reference(child.ordinal)?),
            "exact same ordered source child, without equality deduplication"
        );
    }
    Ok(())
}

fn edges(
    graph: &Graph<'_>,
    references: &[Value],
    original: &Configuration,
    parent: usize,
    edge: impl Fn(usize) -> Edge,
) -> TestResult {
    let children: Vec<_> = original
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.parent == Some(parent))
        .collect();
    assert_eq!(
        references.len(),
        children.len(),
        "all real array slots including null/error/equal values"
    );
    for (slot, (reference, (_index, source))) in references.iter().zip(children).enumerate() {
        assert_eq!(source.edge, edge(slot), "original source slot order");
        assert_eq!(
            reference,
            &graph.owner.reference(source.ordinal)?,
            "exact original owned occurrence reference"
        );
    }
    Ok(())
}

fn error(
    actual: &Value,
    ordinal: u64,
    structure: u32,
    instance: u32,
    evidence: &Evidence<'_>,
) -> TestResult {
    assert_eq!(
        actual.pointer("/value/state"),
        Some(&json!("unreadable")),
        "a genuine decoder error remains semantically unreadable"
    );
    assert_eq!(
        actual.pointer("/value/value"),
        Some(&Value::Null),
        "no successful source payload is fabricated"
    );
    assert_eq!(
        actual.pointer("/originalError/type"),
        Some(&json!("invalidPointer")),
        "exact typed original error"
    );
    for (name, expected) in [("structIndex", structure), ("instanceIndex", instance)] {
        let cell = actual
            .pointer(&format!("/originalError/{name}"))
            .ok_or("complete original error parameter")?;
        assert_eq!(
            raw::present(cell)?,
            &json!(expected),
            "original malformed pointer parameter"
        );
        evidence.value(
            cell,
            ordinal,
            &format!("$error.{name}"),
            SourceUseRole::Payload,
        )?;
    }
    Ok(())
}
