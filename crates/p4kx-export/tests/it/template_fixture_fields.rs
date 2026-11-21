// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Context;

pub(crate) fn input<'catalog>(
    graph: &dyn Context<'catalog>,
    name: &str,
) -> TestResult<Option<&'catalog Value>> {
    let selected = graph
        .projection()
        .pointer(&format!("/inputs/{name}"))
        .ok_or("the actual fixed contextual input binding")?;
    let declaration = selected
        .get("declaration")
        .ok_or("the original declaration binding")?;
    match declaration.get("state").and_then(Value::as_str) {
        Some("present") => Ok(Some(
            graph.bound(
                declaration
                    .get("field")
                    .ok_or("actual original field navigation")?,
            )?,
        )),
        Some("absent") => {
            assert_eq!(
                declaration.get("sourceSetComplete"),
                Some(&json!(true)),
                "a missing declaration has complete actual field-survey evidence"
            );
            assert!(
                declaration.get("field").is_none(),
                "absence invents no original datum"
            );
            Ok(None)
        }
        _ => Err("closed original declaration states".into()),
    }
}

pub(crate) fn structure<'catalog>(
    graph: &dyn Context<'catalog>,
    node: &'catalog Value,
) -> TestResult<&'catalog Value> {
    if node.pointer("/value/value/type") == Some(&json!("struct")) {
        return Ok(node);
    }
    let mut found = None;
    for reference in raw::elements(node)? {
        let value = graph.owner().follow(reference)?;
        if value.pointer("/value/value/type") == Some(&json!("struct")) && found.is_none() {
            found = Some(value);
        }
    }
    found.ok_or_else(|| "the actual established first Struct selection".into())
}

pub(crate) fn required_input<'catalog>(
    graph: &dyn Context<'catalog>,
    name: &str,
) -> TestResult<&'catalog Value> {
    input(graph, name)?.ok_or_else(|| "required original input declaration".into())
}

pub(crate) fn scalar<'node>(node: &'node Value, kind: &str) -> TestResult<&'node Value> {
    raw::payload(node, kind)?
        .get("value")
        .ok_or_else(|| "actual typed original scalar".into())
}

pub(crate) fn number(node: &Value) -> TestResult<&Value> {
    let number = scalar(node, "number")?;
    match number.get("type").and_then(Value::as_str) {
        Some("f32" | "f64") => number
            .get("finite")
            .filter(|value| !value.is_null())
            .ok_or_else(|| "this controlled original number is finite".into()),
        Some("int8" | "int16" | "int32" | "int64" | "uint8" | "uint16" | "uint32" | "uint64") => {
            number
                .get("value")
                .ok_or_else(|| "the exact original integral value".into())
        }
        _ => Err("the actual original numeric width".into()),
    }
}

pub(crate) fn text(node: &Value) -> TestResult<&str> {
    if node.pointer("/value/value/type") == Some(&json!("enum")) {
        return scalar(node, "enum")?
            .as_str()
            .ok_or_else(|| "original enum label".into());
    }
    crate::mission_template_graph::text(node)
}

pub(crate) fn vocabulary<'catalog>(
    graph: &dyn Context<'catalog>,
    node: &Value,
) -> TestResult<&'catalog Value> {
    let reference = graph
        .owner()
        .reference(crate::flow_owner_graph::ordinal(node)?)?;
    let rows = raw::array(graph.projection(), "vocabularies")?;
    let selected: Vec<_> = rows
        .iter()
        .filter(|row| row.get("source") == Some(&reference))
        .collect();
    assert_eq!(
        selected.len(),
        1,
        "one contextual meaning for this actual source occurrence"
    );
    raw::present(
        selected
            .first()
            .ok_or("actual vocabulary projection")?
            .get("meaning")
            .ok_or("independently sourced vocabulary meaning")?,
    )
}

pub(crate) fn elements<'catalog>(
    graph: &dyn Context<'catalog>,
    node: &Value,
) -> TestResult<Vec<&'catalog Value>> {
    raw::elements(node)?
        .iter()
        .map(|reference| graph.owner().follow(reference))
        .collect()
}

pub(crate) fn reference(node: &Value, expected: &str) -> TestResult {
    let reference = scalar(node, "recordReference")?;
    assert_eq!(
        reference.get("status"),
        Some(&json!("present")),
        "the genuine target is present"
    );
    assert_eq!(
        raw::present(reference.get("guid").ok_or("actual sourced target GUID")?)?,
        &json!(expected),
        "the selected target GUID is exact"
    );
    Ok(())
}

pub(crate) fn absent_field(node: &Value, name: &str) -> TestResult {
    let fields = raw::array(raw::payload(node, "struct")?, "fields")?;
    assert!(
        fields
            .iter()
            .all(|field| field.pointer("/name/value") != Some(&json!(name))),
        "this actual Struct has no such authored declaration"
    );
    Ok(())
}

pub(crate) fn scalar_fields(
    graph: &dyn Context<'_>,
    node: &Value,
    expected: &[(&str, &str, Value)],
) -> TestResult {
    for (name, kind, expected) in expected {
        assert_eq!(
            scalar(graph.field(node, name)?, kind)?,
            expected,
            "the exact typed original field {name}"
        );
    }
    Ok(())
}

pub(crate) fn numeric_fields(
    graph: &dyn Context<'_>,
    node: &Value,
    expected: &[(&str, Value)],
) -> TestResult {
    for (name, expected) in expected {
        assert_eq!(
            number(graph.field(node, name)?)?,
            expected,
            "exact original numeric field {name}"
        );
    }
    Ok(())
}
