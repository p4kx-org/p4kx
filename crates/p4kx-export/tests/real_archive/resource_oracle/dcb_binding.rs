// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeOrigin, InputScope, SourceInput, SourceUses};
use serde_json::Value;

use super::super::{Failure, Result};

pub(super) fn dcb_source<'catalog>(
    catalog: &'catalog Value,
    origin: &DataForgeOrigin,
) -> Result<&'catalog str> {
    let DataForgeOrigin::Archive(origin) = origin else {
        return Err(Failure::check("archive-dcb-origin"));
    };
    let sources = SourceUses::with_compact_coordinates();
    let scope = sources
        .observe(SourceInput::Archive(origin), InputScope::Payload)
        .map_err(|error| Failure::source("original-source-observation", error))?;
    let expected = serde_json::to_value(
        sources
            .freeze()
            .map_err(|error| Failure::source("original-source-freeze", error))?,
    )
    .map_err(|error| Failure::source("original-source-contract", error))?;
    let expected_id = scope.source_ref().as_str();
    let actual_sources = inventory(catalog, "/provenance/sources")?;
    let actual_uses = inventory(catalog, "/provenance/uses")?;
    compare_rows(inventory(&expected, "/sources")?, actual_sources)?;
    compare_rows(inventory(&expected, "/uses")?, actual_uses)?;
    observed(actual_sources, expected_id)?["id"]
        .as_str()
        .ok_or_else(|| Failure::check("physical-dcb-id"))
}

fn inventory<'a>(value: &'a Value, path: &str) -> Result<&'a [Value]> {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| Failure::check("physical-source-contract-array"))
}

fn compare_rows(expected: &[Value], actual: &[Value]) -> Result<()> {
    for original in expected {
        let id = original["id"]
            .as_str()
            .ok_or_else(|| Failure::check("original-source-id"))?;
        if !matches_contract(original, observed(actual, id)?) {
            return Err(Failure::check("original-physical-source-mismatch"));
        }
    }
    Ok(())
}

fn observed<'a>(rows: &'a [Value], id: &str) -> Result<&'a Value> {
    let mut matching = rows.iter().filter(|row| row["id"].as_str() == Some(id));
    let first = matching
        .next()
        .ok_or_else(|| Failure::check("original-source-row-missing"))?;
    if matching.next().is_some() {
        return Err(Failure::check("duplicate-original-source-row"));
    }
    Ok(first)
}

fn matches_contract(expected: &Value, actual: &Value) -> bool {
    let mut pending = vec![(expected, actual)];
    while let Some((expected, actual)) = pending.pop() {
        match (expected, actual) {
            (Value::Object(fields), Value::Object(object)) => {
                for (name, value) in fields {
                    let Some(found) = object.get(name) else {
                        return false;
                    };
                    pending.push((value, found));
                }
            }
            (Value::Array(values), Value::Array(array)) if values.len() == array.len() => {
                pending.extend(values.iter().zip(array));
            }
            (Value::Array(_) | Value::Object(_), _) => return false,
            (expected, actual) if expected != actual => return false,
            _ => {}
        }
    }
    true
}
