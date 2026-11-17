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

pub(super) fn escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}
