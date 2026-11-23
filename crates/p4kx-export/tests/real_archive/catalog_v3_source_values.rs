// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::Guid;
use serde_json::Value;

use super::TestResult;

/// Validates immediate state/value/origin metadata before reading a typed caller's payload.
pub(crate) fn source_value(carrier: &Value) -> TestResult<Option<&Value>> {
    source_value_with_set(carrier, false)
}

pub(crate) fn source_value_with_set(
    carrier: &Value,
    has_shared_uses: bool,
) -> TestResult<Option<&Value>> {
    origin(carrier, has_shared_uses)?;
    let value = carrier.get("value").ok_or("required source value")?;
    match carrier.get("state").and_then(Value::as_str) {
        Some("present") => Ok(Some(value)),
        Some("absent" | "null" | "unreadable" | "unsupported" | "invalid") if value.is_null() => {
            Ok(None)
        }
        _ => Err("invalid source state/value correlation".into()),
    }
}

fn origin(carrier: &Value, has_shared_uses: bool) -> TestResult<()> {
    let complete = carrier
        .get("sourceSetComplete")
        .and_then(Value::as_bool)
        .ok_or("source-set completeness")?;
    let marker = carrier
        .get("containsEncryptedSource")
        .ok_or("direct source marker")?;
    if !marker.is_null() && !marker.is_boolean() {
        return Err("invalid direct source marker".into());
    }
    if marker.as_bool() == Some(false) && !complete {
        return Err("false source marker requires complete contributors".into());
    }
    let references = carrier
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("required source-use references")?;
    let empty_marker = if complete { Some(false) } else { None };
    if references.is_empty() && !has_shared_uses && marker.as_bool() != empty_marker {
        return Err("empty contributor origin has an inconsistent marker".into());
    }
    let mut seen = HashSet::new();
    for reference in references {
        let reference = reference
            .as_str()
            .filter(|text| !text.is_empty())
            .ok_or("source-use reference string")?;
        if !seen.insert(reference) {
            return Err("duplicate immediate source-use reference".into());
        }
    }
    Ok(())
}

/// Requires an actual present source result, including nullable derived choices.
pub(crate) fn present(carrier: &Value) -> TestResult<&Value> {
    source_value(carrier)?.ok_or_else(|| "required present source result".into())
}

/// Retains a present nullable GUID choice without turning missing/wrong states into null.
pub(crate) fn nullable_guid(carrier: &Value) -> TestResult<Option<&str>> {
    match present(carrier)? {
        Value::Null => Ok(None),
        Value::String(text) => {
            let parsed: Guid = text.parse()?;
            if parsed.to_string() != *text {
                return Err("nullable GUID must be canonical lowercase text".into());
            }
            Ok(Some(text))
        }
        _ => Err("nullable GUID choice must be a string or null".into()),
    }
}

pub(super) fn record_header(row: &Value) -> TestResult<()> {
    for name in [
        "guid",
        "recordName",
        "className",
        "recordPath",
        "recordIndex",
        "sourceType",
        "fileName",
        "metadataTag",
    ] {
        let value = source_value(row.get(name).ok_or("required category header carrier")?)?;
        if let Some(value) = value {
            let valid = if name == "recordIndex" {
                value
                    .as_u64()
                    .is_some_and(|index| u32::try_from(index).is_ok())
            } else {
                value.is_string()
            };
            if !valid {
                return Err("Record header source payload type".into());
            }
        }
    }
    Ok(())
}
