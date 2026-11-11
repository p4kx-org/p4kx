// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::TestResult;

pub(crate) fn present_value<'row>(row: &'row Value, field: &str) -> TestResult<&'row Value> {
    check::cell(row, field, "present")?
        .get("value")
        .ok_or_else(|| "present Reference datum".into())
}

pub(crate) fn absent_value<'row>(row: &'row Value, field: &str) -> TestResult<&'row Value> {
    let cell = check::cell(row, field, "absent")?;
    check::independent(cell)?;
    cell.get("value")
        .ok_or_else(|| "absent Reference datum".into())
}

pub(crate) fn text<'document>(
    catalog: &'document Value,
    row: &Value,
    field: &str,
) -> TestResult<&'document str> {
    let cell = check::cell(row, field, "present")?;
    check::text_value(check::at(catalog, "/exportHealth")?, cell)?
        .as_str()
        .ok_or_else(|| "same-report Reference text".into())
}

pub(crate) fn by_field<'document>(
    catalog: &'document Value,
    rows: &'document [Value],
    field: &str,
) -> TestResult<&'document Value> {
    for row in rows {
        if text(catalog, row, "field")? == field {
            return Ok(row);
        }
    }
    Err(format!("actual unresolved Reference field {field}").into())
}

pub(crate) fn any(
    rows: &[Value],
    mut predicate: impl FnMut(&Value) -> TestResult<bool>,
) -> TestResult<bool> {
    for row in rows {
        if predicate(row)? {
            return Ok(true);
        }
    }
    Ok(false)
}
