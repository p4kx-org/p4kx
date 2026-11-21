// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::TestResult;

type Coordinate = (String, Vec<String>, String, String);

pub(crate) fn body(ordinal: u64, field: &str, role: &str) -> Coordinate {
    (
        "dataforgeValue".to_owned(),
        vec!["1".to_owned(), ordinal.to_string()],
        field.to_owned(),
        role.to_owned(),
    )
}

pub(crate) fn header(index: u32, field: &str, role: &str) -> Coordinate {
    (
        "dataforgeRecordHeader".to_owned(),
        vec![index.to_string()],
        field.to_owned(),
        role.to_owned(),
    )
}

pub(crate) fn exact(catalog: &Value, cell: &Value, expected: &[Coordinate]) -> TestResult {
    let used = check::uses(catalog, cell)?;
    let actual = used
        .iter()
        .map(|used| coordinate(catalog, used))
        .collect::<TestResult<BTreeSet<_>>>()?;
    let expected: BTreeSet<_> = expected.iter().cloned().collect();
    if used.len() != actual.len() || actual != expected {
        return Err(format!(
            "wrong complete original contribution set: {actual:?}, expected {expected:?}"
        )
        .into());
    }
    Ok(())
}

fn coordinate(catalog: &Value, used: &Value) -> TestResult<Coordinate> {
    assert_eq!(
        used.get("scope"),
        Some(&json!("payload")),
        "DCB headers and body facts are payload scope"
    );
    let source = check::source(catalog, used)?;
    assert_eq!(
        source.get("kind"),
        Some(&json!("archiveEntry")),
        "the actual opened Archive supplies these facts"
    );
    assert_eq!(
        source.pointer("/path/value"),
        Some(&json!("Data\\Game2.dcb")),
        "the actual original DCB supplies each contribution"
    );
    assert_eq!(
        check::at(used, "/location/type")?,
        &json!("subresource"),
        "actual original coordinates are typed"
    );
    let namespace = check::at(used, "/location/namespace")?
        .as_str()
        .ok_or("original namespace")?;
    let indices = check::at(used, "/location/indices")?
        .as_array()
        .ok_or("original indices")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "decimal source index".into())
        })
        .collect::<TestResult<Vec<_>>>()?;
    let field = check::at(used, "/location/field")?
        .as_str()
        .ok_or("original field")?;
    let role = used
        .get("role")
        .and_then(Value::as_str)
        .ok_or("actual use role")?;
    Ok((
        namespace.to_owned(),
        indices,
        field.to_owned(),
        role.to_owned(),
    ))
}
