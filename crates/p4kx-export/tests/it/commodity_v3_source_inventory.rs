// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, TestResult};

/// An original body site or a separately addressed Record-header field.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SourceField {
    Body(String),
    Header(String),
}

pub(crate) fn body(path: &str) -> SourceField {
    SourceField::Body(path.to_owned())
}
pub(crate) fn header(field: &str) -> SourceField {
    SourceField::Header(field.to_owned())
}

/// One original source use with explicit body/header distinction and role.
pub(crate) type Use = (u64, SourceField, String);

/// Resolves each immediate use through the actual product inventory.
pub(crate) fn uses(catalog: &Value, carrier: &Value) -> TestResult<BTreeSet<Use>> {
    let references = fixture::at(carrier, "/sourceUsesRefs")?
        .as_array()
        .ok_or("immediate source uses")?;
    let inventory = fixture::at(catalog, "/provenance/uses")?
        .as_array()
        .ok_or("use inventory")?;
    let sources = fixture::at(catalog, "/provenance/sources")?
        .as_array()
        .ok_or("physical sources")?;
    let mut seen = BTreeSet::new();
    let mut result = BTreeSet::new();
    for reference in references {
        let id = reference.as_str().ok_or("source-use id")?;
        assert!(seen.insert(id), "immediate use references are distinct");
        let mut matching = inventory
            .iter()
            .filter(|used| used.get("id").and_then(Value::as_str) == Some(id));
        let used = matching.next().ok_or("closed use reference")?;
        assert!(
            matching.next().is_none(),
            "each use reference has exactly one inventory row"
        );
        let source_id = fixture::at(used, "/source")?;
        let source = sources
            .iter()
            .find(|source| source.get("id") == Some(source_id))
            .ok_or("closed physical source")?;
        result.insert(physical_use(used, source)?);
    }
    Ok(result)
}

fn physical_use(used: &Value, source: &Value) -> TestResult<Use> {
    assert_eq!(
        fixture::at(source, "/kind")?,
        &json!("archiveEntry"),
        "these facts come from the actual Archive DCB"
    );
    assert_eq!(
        fixture::at(source, "/path/value")?,
        &json!("Data\\Game2.dcb"),
        "no locale or host path is promoted to a DCB fact"
    );
    assert_eq!(
        fixture::at(used, "/scope")?,
        &json!("payload"),
        "the DCB byte domain is explicit"
    );
    let role = fixture::at(used, "/role")?.as_str().ok_or("source role")?;
    assert!(
        matches!(role, "payload" | "selector" | "conversion"),
        "attempted and untouched references are not actual contributors"
    );
    let (record_index, field) = physical_field(used)?;
    Ok((record_index, field, role.to_owned()))
}
fn physical_field(used: &Value) -> TestResult<(u64, SourceField)> {
    let field = fixture::at(used, "/location/field")?
        .as_str()
        .ok_or("original field")?;
    match fixture::at(used, "/location/type")?.as_str() {
        Some("dataForge") => Ok((
            fixture::at(used, "/location/recordIndex")?
                .as_u64()
                .ok_or("body Record index")?,
            body(field),
        )),
        Some("subresource") => {
            assert_eq!(
                fixture::at(used, "/location/namespace")?,
                &json!("dataforgeRecordHeader"),
                "the only admitted subresource is the original Record-header namespace"
            );
            let indices = fixture::at(used, "/location/indices")?
                .as_array()
                .ok_or("header position")?;
            assert_eq!(
                indices.len(),
                1,
                "one original physical Record index owns the header"
            );
            let index = indices
                .first()
                .and_then(Value::as_str)
                .ok_or("decimal header index")?
                .parse::<u32>()?;
            assert!(
                matches!(
                    field,
                    "guid"
                        | "recordName"
                        | "recordIndex"
                        | "sourceType"
                        | "fileName"
                        | "metadataTag"
                ),
                "a derived name/path is never invented as a stored header field"
            );
            Ok((u64::from(index), header(field)))
        }
        _ => Err("unexpected physical source coordinate".into()),
    }
}

/// Checks that every source-use reference closes in the catalog inventory.
pub(crate) fn reference_closure(catalog: &Value) -> TestResult {
    let ids: BTreeSet<_> = fixture::at(catalog, "/provenance/uses")?
        .as_array()
        .ok_or("use inventory")?
        .iter()
        .map(|row| {
            fixture::at(row, "/id")?
                .as_str()
                .ok_or_else(|| "use inventory id is not a string".into())
        })
        .collect::<TestResult<_>>()?;
    walk_reference_closure(catalog, &ids)
}

fn walk_reference_closure(value: &Value, ids: &BTreeSet<&str>) -> TestResult {
    if let Some(references) = value.get("sourceUsesRefs").and_then(Value::as_array) {
        for reference in references {
            assert!(
                ids.contains(reference.as_str().ok_or("use id")?),
                "every source reference closes"
            );
        }
    }
    match value {
        Value::Object(object) => {
            for child in object.values() {
                walk_reference_closure(child, ids)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                walk_reference_closure(child, ids)?;
            }
        }
        _ => {}
    }
    Ok(())
}
