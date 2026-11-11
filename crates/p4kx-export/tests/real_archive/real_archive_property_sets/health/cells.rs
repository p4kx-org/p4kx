// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::SourceValueState;
use serde_json::Value;

use super::super::OracleResult;
use super::super::error::{Failure, OutputFault};

pub(super) struct Cell<'value> {
    pub(super) state: SourceValueState,
    pub(super) value: Option<&'value Value>,
}

pub(super) const fn fault() -> Failure {
    Failure::OutputParse(OutputFault::HealthShape)
}

pub(super) fn read(value: &Value) -> OracleResult<Cell<'_>> {
    let object = value.as_object().ok_or_else(fault)?;
    if object.len() != 5 {
        return Err(fault());
    }
    let state = match object.get("state").and_then(Value::as_str) {
        Some("present") => SourceValueState::Present,
        Some("absent") => SourceValueState::Absent,
        Some("null") => SourceValueState::Null,
        Some("unreadable") => SourceValueState::Unreadable,
        Some("unsupported") => SourceValueState::Unsupported,
        Some("invalid") => SourceValueState::Invalid,
        _ => return Err(fault()),
    };
    origin(value)?;
    let stored = object.get("value").ok_or_else(fault)?;
    let present = state == SourceValueState::Present;
    if present == stored.is_null() {
        return Err(fault());
    }
    Ok(Cell {
        state,
        value: present.then_some(stored),
    })
}

fn origin(cell: &Value) -> OracleResult {
    let marker = cell.get("containsEncryptedSource").ok_or_else(fault)?;
    if !marker.is_null() && !marker.is_boolean() {
        return Err(fault());
    }
    let complete = cell
        .get("sourceSetComplete")
        .and_then(Value::as_bool)
        .ok_or_else(fault)?;
    let refs = cell
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or_else(fault)?;
    let mut unique = HashSet::new();
    for reference in refs {
        let reference = reference
            .as_str()
            .filter(|reference| !reference.is_empty())
            .ok_or_else(fault)?;
        if !unique.insert(reference) {
            return Err(fault());
        }
    }
    if marker == &Value::Bool(false) && !complete {
        return Err(fault());
    }
    if refs.is_empty() {
        let expected = if complete {
            Value::Bool(false)
        } else {
            Value::Null
        };
        if marker != &expected {
            return Err(fault());
        }
    }
    Ok(())
}

pub(super) fn field<'value>(row: &'value Value, name: &str) -> OracleResult<Cell<'value>> {
    read(row.get(name).ok_or_else(fault)?)
}

pub(super) fn index(cell: &Cell<'_>) -> OracleResult<u32> {
    let value = cell.value.and_then(Value::as_u64).ok_or_else(fault)?;
    u32::try_from(value).map_err(|_| fault())
}

pub(super) fn string(cell: &Cell<'_>) -> OracleResult {
    cell.value.and_then(Value::as_str).ok_or_else(fault)?;
    Ok(())
}

pub(super) fn text(cell: &Cell<'_>, texts: &[Value]) -> OracleResult {
    let Some(value) = cell.value else {
        return Ok(());
    };
    let object = value.as_object().ok_or_else(fault)?;
    if object.len() != 1 {
        return Err(fault());
    }
    let digits = object
        .get("atomIndex")
        .and_then(Value::as_str)
        .ok_or_else(fault)?;
    if digits.is_empty()
        || (digits.len() > 1 && digits.starts_with('0'))
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(fault());
    }
    let index: u64 = digits.parse().map_err(|_| fault())?;
    let index = usize::try_from(index).map_err(|_| fault())?;
    let stored = read(texts.get(index).ok_or_else(fault)?)?;
    string(&stored)
}
