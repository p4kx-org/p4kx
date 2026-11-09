// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::Value;

use super::{TestResult, identity, source_value};

pub(super) fn check(row: &Value) -> TestResult<()> {
    let categories = row
        .get("categories")
        .ok_or("required category-list array carrier")?;
    let Some(values) = source_value(categories)? else {
        return Ok(());
    };
    for slot in values.as_array().ok_or("present category-list array")? {
        if let Some(reference) = source_value(slot)? {
            reference_fields(reference)?;
        }
    }
    Ok(())
}

fn reference_fields(reference: &Value) -> TestResult<()> {
    identity(reference, Kind::BlueprintCategoryLists)?;
    let index = source_value(
        reference
            .get("targetRecordIndex")
            .ok_or("target index carrier")?,
    )?;
    let target = source_value(reference.get("targetType").ok_or("target type carrier")?)?;
    let path = source_value(reference.get("recordPath").ok_or("target path carrier")?)?;
    match reference.get("status").and_then(Value::as_str) {
        Some("present") => {
            if index
                .and_then(Value::as_u64)
                .is_none_or(|index| u32::try_from(index).is_err())
                || !target.is_some_and(Value::is_string)
                || !path.is_some_and(Value::is_string)
            {
                return Err(
                    "resolved category-list reference requires actual typed target headers".into(),
                );
            }
        }
        Some("dangling" | "unexpectedTargetType") => {
            if index.is_some() || target.is_some() || path.is_some() {
                return Err(
                    "unresolved category-list reference cannot invent target headers".into(),
                );
            }
        }
        _ => return Err("closed category-list reference status".into()),
    }
    Ok(())
}
