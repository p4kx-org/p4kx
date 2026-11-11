// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::ExportDiagnosticExample;
use serde_json::{Value, json};

use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::TestResult;
use crate::mission_property_remaining_source::{Coordinate, Original};

pub(crate) fn check(
    document: &Value,
    example: &ExportDiagnosticExample,
    actual: &Original,
) -> TestResult {
    exact(
        document,
        &serde_json::to_value(example.source_type())?,
        &[(actual.source_type_ordinal, "$type", "selector")],
    )?;
    exact(
        document,
        &serde_json::to_value(example.field())?,
        &actual.field_uses,
    )?;
    origin::closure(document)
}

fn exact(document: &Value, cell: &Value, expected: &[Coordinate]) -> TestResult {
    let used = origin::uses(document, cell)?;
    let actual = used
        .iter()
        .map(|used| {
            assert_eq!(
                used.get("scope"),
                Some(&json!("payload")),
                "original DCB payload facts"
            );
            assert_eq!(
                used.pointer("/location/namespace"),
                Some(&json!("dataforgeValue")),
                "canonical physical coordinate"
            );
            let indices = used
                .pointer("/location/indices")
                .and_then(Value::as_array)
                .ok_or("actual original indices")?;
            assert_eq!(
                indices.first(),
                Some(&json!("0")),
                "actual original Record index"
            );
            assert_eq!(indices.len(), 2, "record and occurrence coordinate only");
            let ordinal: u64 = indices
                .get(1)
                .and_then(Value::as_str)
                .ok_or("actual source ordinal")?
                .parse()?;
            let field = used
                .pointer("/location/field")
                .and_then(Value::as_str)
                .ok_or("actual original part")?;
            let role = used
                .get("role")
                .and_then(Value::as_str)
                .ok_or("actual original role")?;
            Ok((ordinal, field.to_owned(), role.to_owned()))
        })
        .collect::<TestResult<BTreeSet<_>>>()?;
    let expected: BTreeSet<_> = expected
        .iter()
        .map(|(ordinal, field, role)| (*ordinal, (*field).to_owned(), (*role).to_owned()))
        .collect();
    assert_eq!(
        used.len(),
        actual.len(),
        "no duplicate or extra contributor identities"
    );
    assert_eq!(
        actual, expected,
        "exact original occurrence/part/role set; equal values never rebind origins"
    );
    Ok(())
}
