// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::Value;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

pub(super) fn resolve<'schema>(
    schema: &'schema Value,
    mut node: &'schema Value,
) -> TestResult<&'schema Value> {
    let mut references = BTreeSet::new();
    loop {
        if let Some(reference) = node["$ref"].as_str() {
            let path = reference
                .strip_prefix("#/")
                .ok_or("generated local schema reference")?;
            if !references.insert(path) {
                return Err(format!("reference cycle at {reference}").into());
            }
            node = schema
                .pointer(&reference[1..])
                .ok_or("local reference target")?;
        } else if let Some(branch) = nullable_inner(node) {
            node = branch;
        } else {
            return Ok(node);
        }
    }
}

fn nullable_inner(node: &Value) -> Option<&Value> {
    let alternatives = node["anyOf"].as_array()?.as_slice();
    match alternatives {
        [first, second] if first["type"] == "null" => Some(second),
        [first, second] if second["type"] == "null" => Some(first),
        _ => None,
    }
}

pub(super) fn schema_at<'schema>(schema: &'schema Value, path: &str) -> TestResult<&'schema Value> {
    let mut node = schema;
    for segment in path.trim_start_matches('/').split('/') {
        node = resolve(schema, node)?;
        let child = match segment {
            "[]" => node.get("items"),
            "*" => node.get("additionalProperties"),
            "value" if node.pointer("/properties/value").and_then(Value::as_bool) == Some(true) => {
                present_value(node)
            }
            name => node
                .get("properties")
                .and_then(|fields| fields.get(name.replace("~1", "/").replace("~0", "~"))),
        };
        node = child.ok_or_else(|| format!("missing schema segment {segment} in {path}"))?;
    }
    resolve(schema, node)
}

pub(super) fn present_value(node: &Value) -> Option<&Value> {
    node.get("allOf")?
        .as_array()?
        .iter()
        .find(|case| {
            case.pointer("/if/properties/state/const")
                .and_then(Value::as_str)
                == Some("present")
        })?
        .pointer("/then/properties/value")
}

pub(super) fn escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}
