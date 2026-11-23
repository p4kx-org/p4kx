// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_export::Kind;
use serde_json::Value;

use crate::catalog_v3_consumer::source_values::source_value_with_set;

type TestResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct VariantTarget<'row> {
    pub(crate) guid: &'row str,
    pub(crate) record_index: u32,
}

/// Verifies the variant decision's state, value and required source fields.
pub(crate) fn variant_cell<'row>(row: &'row Value, expected: &str) -> TestResult<&'row Value> {
    let carrier = row.get("variantOf").ok_or("required variant decision")?;
    if carrier.as_object().is_none_or(|object| object.len() != 7)
        || carrier.get("state").and_then(Value::as_str) != Some(expected)
    {
        return Err("invalid variant decision envelope or unexpected state".into());
    }
    let reference = carrier
        .get("sourceUseSetRef")
        .ok_or("required variant set reference")?;
    let shared = !reference.is_null();
    if shared
        && reference
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .is_none()
    {
        return Err("invalid variant set reference".into());
    }
    let failed = carrier
        .get("failedSourceUseRefs")
        .and_then(Value::as_array)
        .ok_or("required failed variant references")?;
    let mut seen = std::collections::HashSet::new();
    for reference in failed {
        let reference = reference
            .as_str()
            .filter(|text| !text.is_empty())
            .ok_or("failed variant reference string")?;
        if !seen.insert(reference) {
            return Err("duplicate failed variant reference".into());
        }
    }
    source_value_with_set(carrier, shared)?;
    let complete = carrier.get("sourceSetComplete").and_then(Value::as_bool);
    match expected {
        "present" if shared && complete == Some(true) => Ok(carrier),
        "absent" if complete == Some(true) => Ok(carrier),
        "unreadable" if complete == Some(false) => Ok(carrier),
        _ => Err("variant decision has inconsistent evidence".into()),
    }
}

/// Reads a known variant decision and preserves the physical target.
pub(crate) fn variant_target(row: &Value, kind: Kind) -> TestResult<Option<VariantTarget<'_>>> {
    if !matches!(kind, Kind::Ships | Kind::Items) {
        return Err("only Ships and Items have variant decisions".into());
    }
    let state = row
        .pointer("/variantOf/state")
        .and_then(Value::as_str)
        .ok_or("required variant decision state")?;
    let carrier = variant_cell(row, state)?;
    match state {
        "absent" => Ok(None),
        "present" => {
            let target = carrier.get("value").ok_or("present variant target")?;
            let guid = complete_variant_field(target, "guid")?
                .as_str()
                .ok_or("variant target GUID text")?;
            let index = complete_variant_field(target, "targetRecordIndex")?
                .as_u64()
                .ok_or("variant target Record index")?;
            Ok(Some(VariantTarget {
                guid,
                record_index: u32::try_from(index)?,
            }))
        }
        _ => Err("variant decision requires proven presence or absence".into()),
    }
}

fn complete_variant_field<'row>(target: &'row Value, name: &str) -> TestResult<&'row Value> {
    let carrier = target.get(name).ok_or("required variant target field")?;
    let value = source_value_with_set(carrier, false)?;
    if carrier.get("sourceSetComplete").and_then(Value::as_bool) != Some(true) {
        return Err("incomplete variant evidence".into());
    }
    value.ok_or_else(|| "present variant target field".into())
}
