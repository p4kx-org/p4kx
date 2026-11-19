// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::is_placeholder_key;
use serde_json::{Map, Value};

/// Collects non-placeholder legacy text candidates in traversal order.
pub(crate) fn keyed_texts<'v>(
    node: &'v Value,
    fields: &[String],
    found: &mut Vec<(&'v str, &'v Value)>,
) {
    match node {
        Value::Object(map) => {
            for (name, value) in map {
                if let Some(key) = candidate_key(name, value, fields) {
                    found.push((key, value));
                }
                keyed_texts(value, fields, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                keyed_texts(item, fields, found);
            }
        }
        _ => {}
    }
}

/// Retains matched values with the ancestor structure needed for traversal.
pub(super) fn retain_candidates(node: Value, fields: &[String]) -> Option<Value> {
    match node {
        Value::Object(children) => {
            let mut retained = Map::new();
            for (name, child) in children {
                let selected = if candidate_key(&name, &child, fields).is_some() {
                    Some(child)
                } else {
                    retain_candidates(child, fields)
                };
                if let Some(selected) = selected {
                    retained.insert(name, selected);
                }
            }
            (!retained.is_empty()).then_some(Value::Object(retained))
        }
        Value::Array(children) => {
            let mut retained = Vec::new();
            for child in children {
                if let Some(selected) = retain_candidates(child, fields) {
                    retained.push(selected);
                }
            }
            (!retained.is_empty()).then_some(Value::Array(retained))
        }
        _ => None,
    }
}

fn candidate_key<'value>(
    name: &str,
    value: &'value Value,
    fields: &[String],
) -> Option<&'value str> {
    if !fields.iter().any(|field| field == name) {
        return None;
    }
    value
        .get("key")
        .and_then(Value::as_str)
        .filter(|key| !is_placeholder_key(key))
}
