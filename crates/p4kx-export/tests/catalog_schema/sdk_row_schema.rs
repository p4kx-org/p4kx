// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::Value;

use super::TestResult;

pub(super) fn compare(
    catalog: &Value,
    catalog_row: &Value,
    query: &Value,
    query_row: &Value,
) -> TestResult {
    let mut compared = BTreeSet::new();
    let mut pending = vec![(catalog_row, query_row, "$row".to_owned())];
    while let Some((expected, actual, path)) = pending.pop() {
        let mut references = Vec::new();
        compare_nodes(expected, actual, &path, &mut references)?;
        for (expected, actual) in references {
            if compared.insert((expected, actual)) {
                let (left, right) = (
                    reference_target(catalog, expected)?,
                    reference_target(query, actual)?,
                );
                pending.push((left, right, expected.to_owned()));
            }
        }
    }
    Ok(())
}

fn compare_nodes<'schema>(
    expected: &'schema Value,
    actual: &'schema Value,
    scope: &str,
    references: &mut Vec<(&'schema str, &'schema str)>,
) -> TestResult {
    match (expected, actual) {
        (Value::Object(left), Value::Object(right)) if left.len() == right.len() => {
            for (key, left) in left {
                let right = right
                    .get(key)
                    .ok_or_else(|| format!("generated row schema lacks {key} at {scope}"))?;
                if key == "$ref" {
                    references.push((
                        left.as_str().ok_or("expected schema reference string")?,
                        right.as_str().ok_or("query schema reference string")?,
                    ));
                } else {
                    compare_nodes(left, right, &format!("{scope}/{key}"), references)?;
                }
            }
        }
        (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
            for (left, right) in left.iter().zip(right) {
                compare_nodes(left, right, scope, references)?;
            }
        }
        (left, right) if left == right => {}
        _ => return Err(format!("generated row schema differs at {scope}").into()),
    }
    Ok(())
}

fn reference_target<'s>(schema: &'s Value, reference: &str) -> TestResult<&'s Value> {
    let path = reference
        .strip_prefix('#')
        .filter(|path| path.starts_with('/'))
        .ok_or("expected local generated schema reference")?;
    schema
        .pointer(path)
        .ok_or_else(|| format!("missing row schema reference {reference}").into())
}
