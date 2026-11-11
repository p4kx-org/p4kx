// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use serde_json::Value;

use super::Result;

pub(crate) fn rows<'catalog>(value: &'catalog Value, path: &str) -> Result<&'catalog [Value]> {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing array at {path}").into())
}

pub(crate) fn indexed<'catalog>(
    value: &'catalog Value,
    path: &str,
    key: &str,
) -> Result<HashMap<&'catalog str, &'catalog Value>> {
    rows(value, path)?
        .iter()
        .map(|row| {
            let id = row[key].as_str().ok_or("row identifier")?;
            Ok((id, row))
        })
        .collect()
}

pub(crate) fn text<'row>(value: &'row Value, path: &str) -> Result<&'row str> {
    value
        .pointer(path)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing text at {path}").into())
}

pub(crate) fn number(value: &Value, path: &str) -> Result<u64> {
    value
        .pointer(path)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing integer at {path}").into())
}
