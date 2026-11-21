// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::broker_flow_graph;
use crate::flow_owner_graph::Owner;
use crate::health_origin_fixture::{TestResult, guid};
use crate::loot_v3_graph as raw;
use crate::mission_template_graph::{self as source, Property};
use crate::scoped_fixture;
use crate::template_canonical_graph as graph;

fn failed<'catalog>(
    catalog: &'catalog Value,
    status: &str,
) -> Result<Property<'catalog>, Box<dyn std::error::Error>> {
    let property = Property::new(catalog, "Scoped_BP")?;
    assert_eq!(
        raw::present(
            property
                .meaning
                .get("kind")
                .ok_or("actual known outer property kind")?
        )?,
        &json!("scopedLocation"),
        "the outer kind remains independent of nested source failure"
    );
    assert_eq!(
        property.meaning.get("status"),
        Some(&json!(status)),
        "the required contextual interpretation cannot become a successful empty or null value"
    );
    assert_eq!(
        catalog.pointer("/exportHealth/status"),
        Some(&json!("incomplete")),
        "diagnostic source retention never establishes complete publication"
    );
    Ok(property)
}

pub(crate) fn unknown(catalog: &Value, field: &str, source_type: &str) -> TestResult {
    let property = failed(catalog, "unsupported")?;
    let filter = property.field(property.node, field)?;
    raw::structure(filter, source_type)?;
    let reference = property
        .owner
        .reference(crate::flow_owner_graph::ordinal(filter)?)?;
    let selected: Vec<_> = raw::array(property.meaning, "scopedFilters")?
        .iter()
        .filter(|row| row.get("source") == Some(&reference))
        .collect();
    assert_eq!(
        selected.len(),
        1,
        "the actual unsupported source occurrence retains its own interpretation"
    );
    assert_eq!(
        raw::present(
            selected
                .first()
                .ok_or("actual unknown filter")?
                .get("kind")
                .ok_or("actual unknown filter kind")?
        )?,
        &Value::Null,
        "an unknown concrete variant does not masquerade as a known filter operation"
    );
    let (owner, broker) = failed_broker(catalog)?;
    raw::structure(graph::struct_field(&owner, broker, field)?, source_type)?;
    Ok(())
}

pub(crate) fn pointer(catalog: &Value, variable: &str, field: &str) -> TestResult {
    let property = if variable == "Scoped_BP" {
        failed(catalog, "unreadable")?
    } else {
        let property = Property::new(catalog, variable)?;
        assert_eq!(
            property.meaning.get("status"),
            Some(&json!("unreadable")),
            "the unreadable search pointer remains a failed contextual configuration"
        );
        property
    };
    invalid_pointer(property.field(property.node, field)?)?;
    if variable == "Scoped_BP" {
        let (owner, broker) = failed_broker(catalog)?;
        invalid_pointer(graph::struct_field(&owner, broker, field)?)?;
    }
    Ok(())
}

fn invalid_pointer(node: &Value) -> TestResult {
    assert_eq!(
        node.pointer("/value/state"),
        Some(&json!("unreadable")),
        "a present invalid pointer is not serialized as an absent configuration"
    );
    assert_eq!(
        node.pointer("/value/value"),
        Some(&Value::Null),
        "there is no fabricated successful source payload"
    );
    let error = node
        .get("originalError")
        .ok_or("the actual invalid pointer error")?;
    assert_eq!(
        error.get("type"),
        Some(&json!("invalidPointer")),
        "the original source failure is typed"
    );
    for (name, expected) in [("structIndex", u32::MAX - 1), ("instanceIndex", 3)] {
        assert_eq!(
            raw::present(error.get(name).ok_or("actual invalid pointer coordinate")?)?,
            &json!(expected),
            "the exact authored invalid pointer words survive"
        );
    }
    Ok(())
}

pub(crate) fn unreadable_enum(catalog: &Value) -> TestResult {
    let property = failed(catalog, "unreadable")?;
    let filter = property.field(property.node, "factionFilter")?;
    raw::structure(filter, "LocationCondition_RequiredFactionHostility")?;
    property.filter_kind(filter, "requiredFactionHostility")?;
    invalid_enum(property.field(filter, "hostility")?);
    let (owner, broker) = failed_broker(catalog)?;
    let filter = graph::struct_field(&owner, broker, "factionFilter")?;
    raw::structure(filter, "LocationCondition_RequiredFactionHostility")?;
    invalid_enum(graph::struct_field(&owner, filter, "hostility")?);
    Ok(())
}

fn invalid_enum(hostility: &Value) {
    assert_eq!(
        hostility.pointer("/value/state"),
        Some(&json!("unreadable")),
        "an unreadable original enum is not an unknown readable enum name"
    );
    assert_eq!(
        hostility.pointer("/value/value"),
        Some(&Value::Null),
        "no enum name is fabricated"
    );
    assert!(
        hostility
            .pointer("/originalError/type")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty()),
        "the exact typed original enum decoder failure remains available"
    );
}

fn failed_broker(catalog: &Value) -> TestResult<(Owner<'_>, &Value)> {
    assert!(
        broker_flow_graph::projections(catalog)?.is_empty(),
        "an unreadable Broker does not publish a contextual configuration"
    );
    let definition = broker_flow_graph::definition(catalog, &guid(scoped_fixture::BROKER))?;
    let owner = Owner::new(definition)?;
    let properties = graph::root_field(&owner, "properties")?;
    let mut selected = None;
    for reference in raw::elements(properties)? {
        let property = owner.follow(reference)?;
        raw::structure(property, "MissionProperty")?;
        let name = graph::struct_field(&owner, property, "missionVariableName")?;
        if source::text(name)? == "Scoped_BP" {
            assert!(
                selected.replace(property).is_none(),
                "one original property occurrence"
            );
        }
    }
    let property = selected.ok_or("actual source property of the refused Broker")?;
    let value = graph::struct_field(&owner, property, "value")?;
    raw::structure(value, "MissionPropertyValue_ScopedLocation")?;
    Ok((owner, value))
}
