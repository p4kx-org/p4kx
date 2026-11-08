// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeOrigin, InputScope, SourceInput, SourceUses};
use serde_json::Value;

use super::super::{Failure, Result};
use super::{MAX_DEPTH, MAX_USES, MAX_VALUE_NODES};

pub(super) fn dcb_source<'catalog>(
    catalog: &'catalog Value,
    origin: &DataForgeOrigin,
) -> Result<&'catalog str> {
    let DataForgeOrigin::Archive(origin) = origin else {
        return Err(Failure::check("archive-dcb-origin"));
    };
    let sources = SourceUses::new();
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
    let mut remaining = MAX_VALUE_NODES;
    compare_rows(
        inventory(&expected, "/sources")?,
        actual_sources,
        &mut remaining,
    )?;
    compare_rows(inventory(&expected, "/uses")?, actual_uses, &mut remaining)?;
    observed(actual_sources, expected_id)?["id"]
        .as_str()
        .ok_or_else(|| Failure::check("physical-dcb-id"))
}

fn inventory<'a>(value: &'a Value, path: &str) -> Result<&'a [Value]> {
    let rows = value
        .pointer(path)
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("physical-source-contract-array"))?;
    if rows.len() > MAX_USES {
        return Err(Failure::check("physical-source-inventory-budget"));
    }
    Ok(rows)
}

fn compare_rows(expected: &[Value], actual: &[Value], remaining: &mut usize) -> Result<()> {
    for original in expected {
        let id = original["id"]
            .as_str()
            .ok_or_else(|| Failure::check("original-source-id"))?;
        if !matches_contract(original, observed(actual, id)?, 0, remaining)? {
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

fn matches_contract(
    expected: &Value,
    actual: &Value,
    depth: usize,
    remaining: &mut usize,
) -> Result<bool> {
    if depth > MAX_DEPTH || *remaining == 0 {
        return Err(Failure::check("original-source-contract-budget"));
    }
    *remaining -= 1;
    match expected {
        Value::Object(fields) => {
            let Some(object) = actual.as_object() else {
                return Ok(false);
            };
            for (name, value) in fields {
                let Some(found) = object.get(name) else {
                    return Ok(false);
                };
                if !matches_contract(value, found, depth + 1, remaining)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Value::Array(values) => {
            let Some(array) = actual
                .as_array()
                .filter(|array| array.len() == values.len())
            else {
                return Ok(false);
            };
            for (value, found) in values.iter().zip(array) {
                if !matches_contract(value, found, depth + 1, remaining)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(expected == actual),
    }
}
