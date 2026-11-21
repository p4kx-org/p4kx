// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::super::{Failure, Result};

/// An original body field, local cargo declaration or separately addressed Record-header field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Coordinate<'a> {
    Body(&'a str),
    Header(&'a str),
    CargoSlot { ordinal: u64, field: &'a str },
}

pub(super) fn read(observation: &Value) -> Result<(u32, Coordinate<'_>)> {
    let location = observation
        .get("location")
        .ok_or_else(|| Failure::check("source-location"))?;
    let field = location
        .get("field")
        .and_then(Value::as_str)
        .ok_or_else(|| Failure::check("source-field"))?;
    match location.get("type").and_then(Value::as_str) {
        Some("dataForge") => {
            let index = location
                .get("recordIndex")
                .and_then(Value::as_u64)
                .and_then(|index| u32::try_from(index).ok())
                .ok_or_else(|| Failure::check("body-record-index"))?;
            Ok((index, Coordinate::Body(field)))
        }
        Some("subresource") => match location.get("namespace").and_then(Value::as_str) {
            Some("dataforgeRecordHeader") => header(location, field),
            Some("dataforgeCargoSlot") => cargo_slot(location, field),
            _ => Err(Failure::check("original-subresource-namespace")),
        },
        _ => Err(Failure::check("original-source-coordinate")),
    }
}

fn header<'a>(location: &'a Value, field: &'a str) -> Result<(u32, Coordinate<'a>)> {
    if !matches!(
        field,
        "guid" | "recordName" | "recordIndex" | "sourceType" | "fileName" | "metadataTag"
    ) {
        return Err(Failure::check("original-header-namespace-field"));
    }
    let indices = location
        .get("indices")
        .and_then(Value::as_array)
        .filter(|indices| indices.len() == 1)
        .ok_or_else(|| Failure::check("header-record-position"))?;
    let index = indices
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| Failure::check("header-decimal-position"))?
        .parse()
        .map_err(|error| Failure::source("header-record-index", error))?;
    Ok((index, Coordinate::Header(field)))
}

fn cargo_slot<'a>(location: &'a Value, field: &'a str) -> Result<(u32, Coordinate<'a>)> {
    if !field.starts_with("defaultCargoContainers.") {
        return Err(Failure::check("cargo-slot-field"));
    }
    let indices = location
        .get("indices")
        .and_then(Value::as_array)
        .filter(|indices| indices.len() == 2)
        .ok_or_else(|| Failure::check("cargo-slot-position"))?;
    let owner = decimal(indices.first())?;
    let owner =
        u32::try_from(owner).map_err(|error| Failure::source("cargo-slot-record-index", error))?;
    let ordinal = decimal(indices.get(1))?;
    Ok((owner, Coordinate::CargoSlot { ordinal, field }))
}

fn decimal(value: Option<&Value>) -> Result<u64> {
    let text = value
        .and_then(Value::as_str)
        .ok_or_else(|| Failure::check("cargo-slot-decimal-position"))?;
    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(Failure::check("cargo-slot-decimal-position"));
    }
    text.parse()
        .map_err(|error| Failure::source("cargo-slot-decimal-position", error))
}
