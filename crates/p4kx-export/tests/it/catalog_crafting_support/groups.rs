// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::error::Error;

use serde_json::Value;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Reads the actual canonical `ResourceGroup` definitions without a fallback view.
pub(crate) fn definitions(catalog: &Value) -> Result<&[Value]> {
    rows(catalog, "/definitions/resourceGroups/rows")
}

/// Requires one nominally unique group in fixtures that declare unique GUIDs.
pub(crate) fn definition<'a>(catalog: &'a Value, guid: &str) -> Result<&'a Value> {
    unique(definitions(catalog)?, guid)
}

/// Reads the original membership declaration of one nominally unique fixture owner.
pub(crate) fn members<'a>(catalog: &'a Value, guid: &str) -> Result<&'a Value> {
    unique(
        rows(catalog, "/declarations/resourceGroupMembers/rows")?,
        guid,
    )
}

/// Reads the chosen parent GUID, including an explicit null decision.
pub(crate) fn parent_guid<'a>(catalog: &'a Value, guid: &str) -> Result<&'a Value> {
    let row = unique(
        rows(catalog, "/projections/resourceGroupParents/rows")?,
        guid,
    )?;
    let value = row
        .pointer("/parent/value")
        .ok_or("required parent payload")?;
    if value.is_null() {
        Ok(value)
    } else {
        value
            .pointer("/guid/value")
            .ok_or_else(|| "required parent GUID".into())
    }
}

/// Reads every source GUID while requiring its immediate canonical carrier.
pub(crate) fn guids(catalog: &Value) -> Result<HashSet<&str>> {
    definitions(catalog)?
        .iter()
        .map(|row| {
            row.pointer("/guid/value")
                .and_then(Value::as_str)
                .ok_or_else(|| "required canonical group GUID".into())
        })
        .collect()
}

fn rows<'a>(catalog: &'a Value, path: &str) -> Result<&'a [Value]> {
    catalog
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("required canonical {path}").into())
}

fn unique<'a>(rows: &'a [Value], guid: &str) -> Result<&'a Value> {
    let mut matching = rows
        .iter()
        .filter(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(guid));
    let row = matching.next().ok_or("required fixture group")?;
    if matching.next().is_some() {
        return Err("the fixture requires one physical group for this GUID".into());
    }
    Ok(row)
}
