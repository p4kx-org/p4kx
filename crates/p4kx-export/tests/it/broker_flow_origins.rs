// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::{BuiltCatalog, ExportDiagnosticExample, ExportHealth};
use p4kx_extraction::{SourceInventory, SourceUseRole};
use serde_json::Value;

use crate::broker_flow_graph::Broker;
use crate::flow_final_origin::Evidence;
use crate::flow_owner_graph;
use crate::health_origin_fixture::{Input, TestResult};

pub(crate) fn check(
    document: &Value,
    inventory: &SourceInventory,
    kind: Input,
    original_count: usize,
) -> TestResult {
    let graph = Broker::new(document)?;
    check_owner(&graph, inventory, kind, original_count)
}

pub(crate) fn check_owner(
    graph: &Broker<'_>,
    inventory: &SourceInventory,
    kind: Input,
    original_count: usize,
) -> TestResult {
    assert_eq!(
        graph.owner.values.len(),
        original_count,
        "every independently counted original occurrence is retained exactly once"
    );
    let input_ref = graph
        .definition
        .get("inputRef")
        .and_then(Value::as_str)
        .ok_or("actual original source navigation")?;
    let evidence = Evidence {
        inventory,
        encrypted: kind.marker(),
        input_ref,
    };
    for (index, node) in graph.owner.values.iter().enumerate() {
        let ordinal = u64::try_from(index)?;
        assert_eq!(
            flow_owner_graph::ordinal(node)?,
            ordinal,
            "the complete original DFS order is preserved without equality deduplication"
        );
        evidence.value(
            node.get("value").ok_or("actual original raw datum")?,
            ordinal,
            "$value",
            SourceUseRole::Payload,
        )?;
    }
    payload_closure(graph.definition, inventory)
}

pub(crate) fn payload_closure(definition: &Value, inventory: &SourceInventory) -> TestResult {
    let input_ref = definition
        .get("inputRef")
        .and_then(Value::as_str)
        .ok_or("physical owner input")?;
    let record_index = definition
        .pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .ok_or("physical owner Record index")?;
    let source = definition
        .get("source")
        .ok_or("physical owner source graph")?;
    let mut retained = BTreeSet::new();
    let mut pending = vec![source];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                if let Some(references) = object.get("sourceUsesRefs").and_then(Value::as_array) {
                    for reference in references {
                        retained.insert(reference.as_str().ok_or("actual source-use reference")?);
                    }
                }
                pending.extend(object.values());
            }
            Value::Array(array) => pending.extend(array),
            _ => {}
        }
    }
    for used in inventory.uses() {
        if used.source().as_str() == input_ref
            && used.role() == SourceUseRole::Payload
            && matches!(used.location(), p4kx_extraction::SourceLocation::Subresource {
                namespace, indices, ..
            } if namespace == "dataforgeValue" && indices.first() == Some(&record_index))
        {
            assert!(
                retained.contains(used.id().as_str()),
                "original raw Payload {} for physical Record {record_index} is absent from its source graph",
                used.id().as_str()
            );
        }
    }
    Ok(())
}

pub(crate) fn unsupported(built: &BuiltCatalog, kind: Input, ordinal: u64) -> TestResult {
    unsupported_type(
        built,
        kind,
        ordinal,
        "missionFlow condition",
        "MissionFlowCondition_Mystery",
    )
}

pub(crate) fn unsupported_type(
    built: &BuiltCatalog,
    kind: Input,
    ordinal: u64,
    operation: &str,
    source_type: &str,
) -> TestResult {
    let health = built.summary().export_health();
    let example = unsupported_example(health, operation, source_type)?;
    let document: Value = serde_json::from_slice(built.json())?;
    let definition =
        crate::broker_flow_graph::definition(&document, crate::broker_flow_fixture::GUID)?;
    let input_ref = definition
        .get("inputRef")
        .and_then(Value::as_str)
        .ok_or("actual original source")?;
    Evidence {
        inventory: built.summary().source_inventory(),
        encrypted: kind.marker(),
        input_ref,
    }
    .value(
        &serde_json::to_value(example.source_type())?,
        ordinal,
        "$type",
        SourceUseRole::Selector,
    )?;
    crate::health_origin_assertions::independent(&serde_json::to_value(example.field())?)?;
    let row = health
        .kinds()
        .iter()
        .find(|row| row.kind == p4kx_export::Kind::MissionBrokerEntries)
        .ok_or("actual Broker health")?;
    assert_eq!(
        (
            row.source_records.selected,
            row.source_records.read_successful,
            row.source_records.projected,
            row.source_records.omitted_after_projection_failure
        ),
        (1, 1, 1, 0),
        "the actual D owner was source-read and emitted successfully"
    );
    assert_eq!(
        row.failed_derived_projections, 1,
        "required C meaning remains explicitly failed"
    );
    Ok(())
}

fn unsupported_example<'a>(
    health: &'a ExportHealth,
    operation: &str,
    source_type: &str,
) -> TestResult<&'a ExportDiagnosticExample> {
    let matching: Vec<_> = health
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.kind == Some(p4kx_export::Kind::MissionBrokerEntries)
                && diagnostic.stage == p4kx_export::ExportDiagnosticStage::DerivedProjection
                && diagnostic.code == p4kx_export::ExportDiagnosticCode::UnsupportedValue
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "the exact actual required interpretation cause"
    );
    let diagnostic = matching.first().ok_or("actual unsupported diagnostic")?;
    assert_eq!(
        diagnostic.count, 1,
        "one actual original failed required interpretation"
    );
    assert_eq!(
        diagnostic.examples().len(),
        1,
        "one actual original unsupported occurrence"
    );
    let example = diagnostic
        .examples()
        .first()
        .ok_or("actual unsupported example")?;
    assert_eq!(
        example.record_index().value(),
        Some(&0),
        "the genuine physical owner"
    );
    assert_eq!(
        example
            .record_guid()
            .value()
            .map(p4kx_export::HealthText::as_str),
        Some(crate::broker_flow_fixture::GUID),
        "the same independently proven original header GUID"
    );
    assert_eq!(
        example.field_text().value(),
        Some(operation),
        "the unchanged technical operation label"
    );
    assert_eq!(
        example.source_type_text().value(),
        Some(source_type),
        "the exact independently proven unknown original type"
    );
    assert!(
        example.target_guid().value().is_none(),
        "no fabricated target GUID"
    );
    Ok(example)
}
