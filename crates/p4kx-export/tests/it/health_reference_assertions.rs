// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{self as fixture, TestResult};
use crate::health_reference_fixture as source;
use crate::health_reference_origins as origins;

pub(crate) fn rows(catalog: &Value) -> TestResult<&[Value]> {
    check::rows(catalog, "/exportHealth/unresolvedReferences")
}

pub(crate) fn by_field<'document>(
    catalog: &'document Value,
    field: &str,
) -> TestResult<&'document Value> {
    let health = check::at(catalog, "/exportHealth")?;
    for row in rows(catalog)? {
        let Some(cell) = row.get("field") else {
            continue;
        };
        if check::text_value(health, cell)? == field {
            return Ok(row);
        }
    }
    Err(format!("actual source reference at {field}").into())
}

pub(crate) fn resolved(
    catalog: &Value,
    row: &Value,
    field: &str,
    ordinal: u64,
    marker: Option<bool>,
) -> TestResult {
    assert_eq!(
        row.get("kind"),
        Some(&json!("manufacturers")),
        "the existing selected kind stays intact"
    );
    assert_eq!(
        row.get("sourceRole"),
        Some(&json!("selected")),
        "the actual owner remains selected"
    );
    assert_eq!(
        row.get("status"),
        Some(&json!("excludedByFilter")),
        "the readable actual target is intentionally filtered"
    );
    owner(catalog, row, marker)?;
    check::present_text(catalog, row, "field", &json!(field), marker)?;
    target(catalog, row, ordinal, marker)?;
    absent(row, "targetLocation")?;
    assert_eq!(
        row.get("sourcePointer"),
        Some(&Value::Null),
        "a GUID Reference has no invented pointer word group"
    );
    Ok(())
}

pub(crate) fn absent(row: &Value, field: &str) -> TestResult {
    let cell = check::cell(row, field, "absent")?;
    check::independent(cell)
}

pub(crate) fn target_binding(catalog: &Value, row: &Value, ordinal: u64) -> TestResult {
    let cell = row
        .get("targetRecordPath")
        .ok_or("target path occurrence")?;
    if cell.get("state") != Some(&json!("present"))
        || cell.get("containsEncryptedSource") != Some(&json!(true))
        || cell.get("sourceSetComplete") != Some(&json!(true))
    {
        return Err("wrong semantic target path state or marker".into());
    }
    if check::text_value(check::at(catalog, "/exportHealth")?, cell)? != &fixture_target() {
        return Err("wrong original target spelling".into());
    }
    origins::exact(
        catalog,
        cell,
        &[
            origins::header(0, "fileName", "conversion"),
            origins::body(ordinal, "$value", "selector"),
        ],
    )
}

fn fixture_target() -> Value {
    json!(source::target_path())
}

fn owner(catalog: &Value, row: &Value, marker: Option<bool>) -> TestResult {
    let index = check::present(row, "recordIndex", &json!(1), marker)?;
    origins::exact(
        catalog,
        index,
        &[origins::header(1, "recordIndex", "selector")],
    )?;
    let guid = check::present(
        row,
        "recordGuid",
        &json!(fixture::guid(source::OWNER)),
        marker,
    )?;
    origins::exact(catalog, guid, &[origins::header(1, "guid", "payload")])?;
    let path = check::present_text(
        catalog,
        row,
        "recordPath",
        &json!(source::OWNER_PATH),
        marker,
    )?;
    origins::exact(
        catalog,
        path,
        &[origins::header(1, "fileName", "conversion")],
    )?;
    let source_type =
        check::present_text(catalog, row, "sourceType", &json!(source::TYPE), marker)?;
    origins::exact(
        catalog,
        source_type,
        &[origins::header(1, "sourceType", "selector")],
    )?;
    Ok(())
}

fn target(catalog: &Value, row: &Value, ordinal: u64, marker: Option<bool>) -> TestResult {
    let target = check::present(
        row,
        "targetGuid",
        &json!(fixture::guid(source::TARGET)),
        marker,
    )?;
    origins::exact(
        catalog,
        target,
        &[origins::body(ordinal, "$value", "payload")],
    )?;
    let index = check::present(row, "targetRecordIndex", &json!(0), marker)?;
    origins::exact(
        catalog,
        index,
        &[
            origins::header(0, "recordIndex", "selector"),
            origins::body(ordinal, "$value", "selector"),
        ],
    )?;
    let path = check::present_text(
        catalog,
        row,
        "targetRecordPath",
        &json!(source::target_path()),
        marker,
    )?;
    origins::exact(
        catalog,
        path,
        &[
            origins::header(0, "fileName", "conversion"),
            origins::body(ordinal, "$value", "selector"),
        ],
    )?;
    let source_type =
        check::present_text(catalog, row, "targetType", &json!(source::TYPE), marker)?;
    origins::exact(
        catalog,
        source_type,
        &[
            origins::header(0, "sourceType", "selector"),
            origins::body(ordinal, "$value", "selector"),
        ],
    )?;
    Ok(())
}
