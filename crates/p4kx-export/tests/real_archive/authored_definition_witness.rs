// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_extraction::{DataForge, Value as Original, ValueError};
use serde_json::{Value, json};

use crate::catalog_v3_consumer as consumer;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) fn verify_array<'source>(
    source: Option<&Original<'source>>,
    row: &Value,
    verify: impl Fn(Option<&Original<'source>>, &Value) -> TestResult,
) -> TestResult {
    let Some(Original::Array(slots)) = source else {
        return verify_state(source, row);
    };
    let payload = consumer::source_value(row)?
        .and_then(Value::as_array)
        .ok_or("expected original array payload")?;
    assert_eq!(
        payload.len(),
        slots.len(),
        "every original array occurrence"
    );
    for (slot, row) in slots.iter().zip(payload) {
        verify(Some(slot), row)?;
    }
    Ok(())
}

pub(crate) fn verify_link(
    forge: &DataForge<'_>,
    expected_type: &str,
    source: Option<&Original<'_>>,
    row: &Value,
) -> TestResult {
    let (guid, status) = match source {
        Some(Original::Reference(target)) => {
            let record = forge
                .record(target.record_index())
                .ok_or("original link target")?;
            let status = if matches_ancestry(record.struct_type()?, expected_type)? {
                "present"
            } else {
                "unexpectedTargetType"
            };
            (target.guid(), status)
        }
        Some(Original::Error(ValueError::DanglingReference { guid })) => (*guid, "dangling"),
        _ => return verify_state(source, row),
    };
    let payload = consumer::source_value(row)?.ok_or("expected original Reference payload")?;
    let actual = consumer::source_value(field(payload, "guid")?)?;
    assert_eq!(
        actual,
        Some(&json!(guid.to_string())),
        "exact Reference GUID"
    );
    assert_eq!(
        field(payload, "status")?,
        &json!(status),
        "actual original link state"
    );
    Ok(())
}

fn matches_ancestry(mut actual: p4kx_extraction::Struct<'_>, expected: &str) -> TestResult<bool> {
    let mut seen = BTreeSet::new();
    while seen.insert(actual.index()) {
        if actual.name()? == expected {
            return Ok(true);
        }
        let Some(parent) = actual.parent() else {
            return Ok(false);
        };
        actual = parent;
    }
    Err("cycle in original target type ancestry".into())
}

pub(crate) fn verify_state(source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let expected = match source {
        None => "absent",
        Some(Original::Null) => "null",
        Some(Original::Error(_)) => "unreadable",
        Some(_) => "invalid",
    };
    assert!(
        consumer::source_value(row)?.is_none(),
        "missing original payload"
    );
    assert_eq!(field(row, "state")?, &json!(expected), "exact source state");
    Ok(())
}

pub(crate) fn field<'row>(row: &'row Value, name: &str) -> TestResult<&'row Value> {
    row.get(name)
        .ok_or_else(|| format!("missing catalog field {name}").into())
}
