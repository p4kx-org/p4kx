// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, TestResult};

/// One physical DataForge use addressed by Record index, field and role.
pub(crate) type Use = (u64, String, String);

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
        fixture::at(used, "/location/type")?,
        &json!("dataForge"),
        "physical facts retain their DataForge location kind"
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
    Ok((
        fixture::at(used, "/location/recordIndex")?
            .as_u64()
            .ok_or("physical Record index")?,
        fixture::at(used, "/location/field")?
            .as_str()
            .ok_or("field path")?
            .to_owned(),
        role.to_owned(),
    ))
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
