// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashSet};

use p4kx_extraction::{DataForge, Record};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as carrier, TestResult};

const HEADER: [&str; 8] = [
    "guid",
    "recordName",
    "className",
    "recordPath",
    "recordIndex",
    "sourceType",
    "fileName",
    "metadataTag",
];

pub(crate) fn rows(catalog: &Value) -> TestResult<&[Value]> {
    let table = carrier::at(catalog, "/definitions/blueprintCategories")?;
    let rows = carrier::at(table, "/rows")?
        .as_array()
        .ok_or("category rows")?;
    match carrier::at(table, "/availability")?.as_str() {
        Some("available") => Ok(rows),
        Some("absent") if rows.is_empty() => Ok(rows),
        _ => Err("category source unavailable or inconsistent".into()),
    }
}

pub(crate) fn class_names(catalog: &Value) -> TestResult<Vec<&str>> {
    rows(catalog)?
        .iter()
        .map(|row| {
            carrier::payload(row, "className")?
                .as_str()
                .ok_or_else(|| "category class carrier".into())
        })
        .collect()
}

pub(crate) fn guids(catalog: &Value) -> TestResult<HashSet<&str>> {
    rows(catalog)?
        .iter()
        .map(|row| {
            carrier::payload(row, "guid")?
                .as_str()
                .ok_or_else(|| "category identity carrier".into())
        })
        .collect()
}

pub(crate) fn assert_header(
    row: &Value,
    source: &DataForgeBuilder,
    guid: &str,
    class_name: &str,
    record_path: &str,
) -> TestResult {
    let actual: BTreeSet<_> = row
        .as_object()
        .ok_or("category header object")?
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        actual,
        BTreeSet::from(HEADER),
        "a source category contains exactly its eight original header cells"
    );
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let record = forge
        .records()
        .find(|record| record.guid().to_string() == guid)
        .ok_or("original category fixture record")?;
    assert_eq!(
        record.record_path()?,
        record_path,
        "the fixed fixture path remains unchanged"
    );
    assert_cells(row, &record, guid, class_name)?;
    assert_tag(row, &record)
}

fn assert_cells(row: &Value, record: &Record<'_>, guid: &str, class_name: &str) -> TestResult {
    let stored_name = record
        .stored_name()?
        .fold(|| None, || None, Some)
        .ok_or("fixture category name")?;
    for (field, expected) in [
        ("guid", json!(guid)),
        ("recordName", json!(stored_name)),
        ("className", json!(class_name)),
        ("recordPath", json!(record.record_path()?)),
        ("recordIndex", json!(record.index())),
        ("sourceType", json!(record.type_name()?)),
        ("fileName", json!(record.stored_file_name()?)),
    ] {
        assert_eq!(
            carrier::payload(row, field)?,
            &expected,
            "the canonical category {field} equals its actual original header"
        );
    }
    Ok(())
}

fn assert_tag(row: &Value, record: &Record<'_>) -> TestResult {
    let (state, tag) = record.stored_tag()?.fold(
        || ("absent", Value::Null),
        || ("null", Value::Null),
        |tag| ("present", json!(tag)),
    );
    assert_eq!(
        carrier::at(row, "/metadataTag/state")?,
        &json!(state),
        "tag declaration state is independent of an empty string"
    );
    assert_eq!(
        carrier::at(row, "/metadataTag/value")?,
        &tag,
        "the original metadata tag is retained without a name placeholder"
    );
    Ok(())
}
