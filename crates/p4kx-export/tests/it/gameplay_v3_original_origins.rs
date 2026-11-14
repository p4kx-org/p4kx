// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::Value;

use crate::commodity_v3_fixture::TestResult;
use crate::quality_v3_origin_support::{self as origins, Domain, Use};

pub(crate) fn body(
    catalog: &Value,
    carrier: &Value,
    record: u32,
    path: &str,
    role: &str,
) -> TestResult {
    if carrier.get("sourceSetComplete").and_then(Value::as_bool) != Some(true) {
        return Err("the positive original-source proof is incomplete".into());
    }
    let expected = BTreeSet::from([Use::new(Domain::Body, u64::from(record), path, role)]);
    if origins::uses(catalog, carrier)? != expected {
        return Err(
            format!("original contribution differs at Record {record}, {path}, {role}").into(),
        );
    }
    Ok(())
}

pub(crate) fn state<'a>(carrier: &'a Value, expected: &str) -> TestResult<Option<&'a Value>> {
    let value = crate::gameplay_v3_consumer::source_value(carrier)?;
    if carrier.get("state").and_then(Value::as_str) != Some(expected) {
        return Err(format!("original source state differs: expected {expected}").into());
    }
    Ok(value)
}
