// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_export::Kind;
use serde_json::Value;

use super::{TestResult, rows};
use crate::catalog_v3_consumer;

/// Requires exact original owner headers and distinct, authored-order component slots.
pub(crate) fn check(catalog: &Value, table: &str, item: &Value) -> TestResult {
    catalog_v3_consumer::identity(item, Kind::Items)?;
    let guid = item.get("guid").ok_or("source Item GUID carrier")?;
    original(guid)?;
    let record = item
        .get("recordIndex")
        .ok_or("physical Item Record carrier")?;
    let index = ordinal(record)?;
    let mut selected = HashSet::new();
    let mut previous = None;
    for row in rows(catalog, table)? {
        let row_record = row
            .get("ownerRecordIndex")
            .ok_or("physical component owner")?;
        if ordinal(row_record)? != index {
            continue;
        }
        if row.get("ownerGuid") != Some(guid) || row_record != record {
            return Err(
                "the component owner does not carry the same original source header".into(),
            );
        }
        let slot = ordinal(
            row.get("componentOrdinal")
                .ok_or("original component slot")?,
        )?;
        if !selected.insert(slot) || previous.is_some_and(|previous| previous >= slot) {
            return Err("physical component slots must be distinct and in authored order".into());
        }
        previous = Some(slot);
    }
    Ok(())
}

fn original(carrier: &Value) -> TestResult<&Value> {
    let value = catalog_v3_consumer::present(carrier)?;
    if carrier.get("sourceSetComplete").and_then(Value::as_bool) != Some(true)
        || carrier
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        return Err("physical owner identity requires its original source evidence".into());
    }
    Ok(value)
}

fn ordinal(carrier: &Value) -> TestResult<u32> {
    let value = original(carrier)?
        .as_u64()
        .ok_or("original physical ordinal")?;
    Ok(u32::try_from(value)?)
}
