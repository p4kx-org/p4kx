// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Context;
use crate::template_fixture_fields as fields;

pub(crate) struct Pool<'catalog> {
    pub(crate) variable: &'catalog str,
    pub(crate) level: &'catalog str,
    pub(crate) identifier: &'catalog Value,
}

pub(crate) fn pools<'catalog>(
    graph: &dyn Context<'catalog>,
    kind: &str,
) -> TestResult<Vec<Pool<'catalog>>> {
    let mut result = Vec::new();
    for binding in raw::array(graph.projection(), "poolBindings")? {
        let reference = binding
            .get("registration")
            .ok_or("actual content registration reference")?;
        let registrations: Vec<_> = raw::array(graph.projection(), "spawnPools")?
            .iter()
            .filter(|row| row.get("source") == Some(reference))
            .collect();
        assert_eq!(
            registrations.len(),
            1,
            "the actual winning source selects one registration"
        );
        let registration = registrations
            .first()
            .ok_or("actual registered original source")?
            .get("result")
            .ok_or("actual registration result")?;
        if registration.get("type").and_then(Value::as_str) != Some(kind) {
            continue;
        }
        if kind != "location" {
            assert_eq!(
                registration.get("empty"),
                Some(&json!(false)),
                "the existing winner-selection rule excludes genuinely empty descriptions"
            );
        }
        let identifier = raw::present(
            registration
                .get(if kind == "location" { "query" } else { "pool" })
                .ok_or("actual independently sourced content id")?,
        )?;
        let variable = graph.bound(
            binding
                .get("variable")
                .ok_or("actual variable field reference")?,
        )?;
        result.push(Pool {
            variable: fields::text(variable)?,
            level: binding
                .get("level")
                .and_then(Value::as_str)
                .ok_or("actual selected source level")?,
            identifier,
        });
    }
    Ok(result)
}

pub(crate) fn objectives<'catalog>(graph: &dyn Context<'catalog>) -> TestResult<&'catalog [Value]> {
    raw::array(graph.projection(), "objectives")
}

pub(crate) fn objective<'catalog>(
    graph: &dyn Context<'catalog>,
    index: usize,
) -> TestResult<&'catalog Value> {
    let row = raw::array(graph.projection(), "objectives")?
        .get(index)
        .ok_or("actual selected objective in original preorder")?;
    graph
        .owner()
        .follow(row.get("source").ok_or("actual physical objective link")?)
}

pub(crate) fn properties<'catalog>(
    graph: &dyn Context<'catalog>,
    objective: Option<usize>,
) -> TestResult<Vec<&'catalog Value>> {
    let objective = match objective {
        Some(index) => Some(
            raw::array(graph.projection(), "objectives")?
                .get(index)
                .and_then(|row| row.get("source"))
                .ok_or("actual selected physical objective")?,
        ),
        None => None,
    };
    Ok(raw::array(graph.projection(), "properties")?
        .iter()
        .filter(|row| row.get("objective") == Some(objective.unwrap_or(&Value::Null)))
        .collect())
}

pub(crate) fn binding_text<'catalog>(
    graph: &dyn Context<'catalog>,
    row: &Value,
    name: &str,
) -> TestResult<Option<&'catalog str>> {
    let binding = row.get(name).ok_or("actual property naming declaration")?;
    match binding.get("state").and_then(Value::as_str) {
        Some("present") => {
            let node = graph.bound(
                binding
                    .get("field")
                    .ok_or("actual naming field reference")?,
            )?;
            Ok(Some(fields::text(node)?))
        }
        Some("absent") => Ok(None),
        _ => Err("closed naming declaration states".into()),
    }
}

pub(crate) fn value<'catalog>(
    graph: &dyn Context<'catalog>,
    property: &Value,
) -> TestResult<&'catalog Value> {
    graph.owner().follow(
        property
            .get("value")
            .ok_or("actual selected property value")?,
    )
}

pub(crate) fn kind(graph: &dyn Context<'_>, property: &Value, expected: &str) -> TestResult {
    assert_eq!(
        property.get("valueKind"),
        Some(&json!(expected)),
        "the actual selected property kind"
    );
    let source = property
        .get("value")
        .ok_or("actual selected original value")?;
    let matching: Vec<_> = raw::array(graph.projection(), "valueMeanings")?
        .iter()
        .filter(|row| row.get("source") == Some(source))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "one interpretation of this original value"
    );
    let row = *matching.first().ok_or("actual value meaning")?;
    assert_eq!(
        raw::present(row.get("kind").ok_or("sourced actual value kind")?)?,
        &json!(expected),
        "the recognized original union arm"
    );
    assert_eq!(
        row.get("status"),
        Some(&json!("recognized")),
        "required interpretation succeeds"
    );
    Ok(())
}

pub(crate) fn context<'catalog>(
    graph: &dyn Context<'catalog>,
    table: &str,
    key: &str,
    node: &Value,
) -> TestResult<&'catalog Value> {
    let reference = graph
        .owner()
        .reference(crate::flow_owner_graph::ordinal(node)?)?;
    let matching: Vec<_> = raw::array(graph.projection(), table)?
        .iter()
        .filter(|row| row.get(key) == Some(&reference))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "one contextual selection for this physical occurrence"
    );
    matching
        .first()
        .copied()
        .ok_or_else(|| "actual occurrence context".into())
}

pub(crate) fn property_target(graph: &dyn Context<'_>, node: &Value, expected: &str) -> TestResult {
    let row = context(graph, "propertyBindings", "caller", node)?;
    let target = row.get("target").ok_or("actual property selection")?;
    assert_eq!(
        target.get("interpretation"),
        Some(&json!("value")),
        "usable original variable"
    );
    assert_eq!(
        fields::text(
            graph.bound(
                target
                    .get("field")
                    .ok_or("selected original target field")?
            )?
        )?,
        expected,
        "the actual target variable remains exact"
    );
    Ok(())
}

pub(crate) fn named<'catalog>(
    graph: &dyn Context<'catalog>,
    variable: &str,
) -> TestResult<&'catalog Value> {
    let mut found = None;
    for property in properties(graph, None)? {
        if binding_text(graph, property, "variable")? == Some(variable) {
            assert!(
                found.is_none(),
                "the controlled name has one actual property occurrence"
            );
            found = Some(property);
        }
    }
    found.ok_or_else(|| format!("actual original property {variable}").into())
}

pub(crate) fn spawn(
    catalog: &Value,
    graph: &dyn Context<'_>,
    property: &Value,
    expected: &str,
    target: &str,
) -> TestResult {
    kind(graph, property, expected)?;
    let source = property.get("value").ok_or("actual property value link")?;
    let matching: Vec<_> = raw::array(graph.projection(), "spawnPools")?
        .iter()
        .filter(|row| row.get("source") == Some(source))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "one registration for this actual selected value"
    );
    let result = matching
        .first()
        .and_then(|row| row.get("result"))
        .ok_or("actual content result")?;
    let result_type = match expected {
        "shipSpawnDescriptions" => "ships",
        "npcSpawnDescriptions" => "npcs",
        "entitySpawnDescriptions" => "entities",
        "location" => "location",
        _ => return Err("this control names a concrete existing spawn family".into()),
    };
    assert_eq!(
        result.get("type"),
        Some(&json!(result_type)),
        "the exact registered family"
    );
    let identifier = raw::present(
        result
            .get(if expected == "location" {
                "query"
            } else {
                "pool"
            })
            .ok_or("actual independently sourced content identifier")?,
    )?;
    assert!(
        raw::array(catalog, target)?
            .iter()
            .any(|row| row.get("guid") == Some(identifier)),
        "the actual typed registration resolves in its existing dataset"
    );
    Ok(())
}
