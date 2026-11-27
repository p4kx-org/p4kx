// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::Index;
use crate::origin_test_support as support;

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(crate) fn field(
    output: &Path,
    index: &Index,
    stream: &str,
    ordinal: usize,
    pointer: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let ordinal = ordinal.to_string();
    support::rows(output, index, "field-origins.jsonl")?
        .into_iter()
        .find(|row| {
            row.get("type").and_then(Value::as_str) == Some("field")
                && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some(stream)
                && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
                && row.get("field").and_then(Value::as_str) == Some(pointer)
        })
        .ok_or_else(|| "actual aggregate field".into())
}

pub(crate) fn references(field: &Value) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("immediate original use references")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "original use reference".into())
        })
        .collect()
}

pub(crate) fn assert_union(parent: &Value, children: &[Value]) -> TestResult {
    let mut expected = BTreeSet::new();
    for child in children {
        expected.extend(references(child)?);
    }
    assert_eq!(
        references(parent)?,
        expected,
        "aggregate uses are the actual child contribution union"
    );
    Ok(())
}

pub(crate) fn assert_contains(parent: &Value, children: &[Value]) -> TestResult {
    let parent = references(parent)?;
    for child in children {
        assert!(
            references(child)?.is_subset(&parent),
            "known original child inputs survive the compound datum"
        );
    }
    Ok(())
}
