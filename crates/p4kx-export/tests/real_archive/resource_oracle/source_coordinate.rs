// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::super::{Failure, Result};

/// An original body field or a separately addressed Record-header field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Coordinate<'a> {
    Body(&'a str),
    Header(&'a str),
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
        Some("subresource") => header(location, field),
        _ => Err(Failure::check("original-source-coordinate")),
    }
}

fn header<'a>(location: &'a Value, field: &'a str) -> Result<(u32, Coordinate<'a>)> {
    if location.get("namespace").and_then(Value::as_str) != Some("dataforgeRecordHeader")
        || !matches!(
            field,
            "guid" | "recordName" | "recordIndex" | "sourceType" | "fileName" | "metadataTag"
        )
    {
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
