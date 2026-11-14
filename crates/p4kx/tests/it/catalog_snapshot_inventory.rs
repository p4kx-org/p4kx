// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;

use serde_json::Value;

type TestResult<T> = Result<T, Box<dyn Error>>;
type Uses<'a> = BTreeMap<&'a str, &'a Value>;

pub(crate) fn check(document: &Value, inventory: &Value) -> TestResult<usize> {
    let uses = uses(inventory)?;
    let mut pending = vec![document];
    let mut count = 0;
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                if let Some(references) = object.get("sourceUsesRefs") {
                    count += references_in(references, &uses)?;
                }
                pending.extend(object.values());
            }
            Value::Array(array) => pending.extend(array),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    Ok(count)
}

fn uses(inventory: &Value) -> TestResult<Uses<'_>> {
    let sources = inventory
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("snapshot sources")?;
    let mut source_ids = BTreeSet::new();
    for source in sources {
        let id = text(source, "id")?;
        if !source_ids.insert(id) {
            return Err("duplicate snapshot source".into());
        }
    }
    if source_ids.is_empty() {
        return Err("missing original snapshot sources".into());
    }
    let mut uses = BTreeMap::new();
    for observed in inventory
        .get("uses")
        .and_then(Value::as_array)
        .ok_or("snapshot uses")?
    {
        if !source_ids.contains(text(observed, "source")?) {
            return Err("use has no source in its own snapshot".into());
        }
        if uses.insert(text(observed, "id")?, observed).is_some() {
            return Err("duplicate snapshot use".into());
        }
    }
    if uses.is_empty() {
        return Err("missing original snapshot uses".into());
    }
    Ok(uses)
}

fn references_in(references: &Value, uses: &Uses<'_>) -> TestResult<usize> {
    let references = references.as_array().ok_or("source reference array")?;
    let mut seen = BTreeSet::new();
    for reference in references {
        let id = reference.as_str().ok_or("source reference text")?;
        if !seen.insert(id) {
            return Err("duplicate datum source reference".into());
        }
        let observed = uses.get(id).ok_or("datum reference outside own snapshot")?;
        if !matches!(
            text(observed, "role")?,
            "payload" | "selector" | "conversion"
        ) {
            return Err("datum references a noncontributing use".into());
        }
    }
    Ok(references.len())
}

fn text<'a>(object: &'a Value, field: &str) -> TestResult<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("snapshot {field}").into())
}
