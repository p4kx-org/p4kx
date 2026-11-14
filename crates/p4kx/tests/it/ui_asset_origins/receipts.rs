// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::TestResult;

pub(super) fn origin<'a>(report: &'a Value, path: &str) -> TestResult<&'a Value> {
    report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or_else(|| format!("missing origin for {path}").into())
}

pub(super) fn uses<'a>(report: &'a Value, path: &str) -> TestResult<Vec<&'a Value>> {
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    origin(report, path)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source use references")?
        .iter()
        .map(|reference| {
            uses.iter()
                .find(|row| row.get("id") == Some(reference))
                .ok_or_else(|| "unresolved source reference".into())
        })
        .collect()
}

pub(super) fn scoped<'a>(rows: &[&'a Value], role: &str, namespace: &str) -> TestResult<&'a Value> {
    rows.iter()
        .copied()
        .find(|row| {
            row.get("role").and_then(Value::as_str) == Some(role)
                && row.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
        })
        .ok_or_else(|| format!("missing {role} use in {namespace}").into())
}

pub(super) fn assert_tag(
    report: &Value,
    path: &str,
    role: &str,
    code: u16,
    expected_offset: &str,
) -> TestResult {
    let inputs = uses(report, path)?;
    let source = scoped(&inputs, role, "gfxTag")?;
    assert_eq!(
        source.pointer("/location/indices/0"),
        Some(&json!(code.to_string())),
        "{path}"
    );
    assert_eq!(
        source.pointer("/location/indices/1"),
        Some(&json!(expected_offset)),
        "{path}"
    );
    Ok(())
}

fn addresses(value: &Value, parent: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(values) => {
            for (field, value) in values {
                let path = format!("{parent}/{field}");
                if parent.is_empty() && matches!(field.as_str(), "fieldOrigins" | "provenance") {
                    continue;
                }
                out.insert(path.clone());
                addresses(value, &path, out);
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                let path = format!("{parent}/{index}");
                out.insert(path.clone());
                addresses(value, &path, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn assert_all_fields(report: &Value) -> TestResult {
    let mut expected = BTreeSet::new();
    addresses(report, "", &mut expected);
    let actual = report["fieldOrigins"].as_object().ok_or("field origins")?;
    assert_eq!(
        expected,
        actual.keys().cloned().collect(),
        "all existing values are addressed"
    );
    Ok(())
}
