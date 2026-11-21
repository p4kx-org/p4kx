// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::support::TestResult;

pub(super) fn origin<'a>(meta: &'a Value, field: &str) -> TestResult<&'a Value> {
    meta.get("fieldOrigins")
        .and_then(|fields| fields.get(field))
        .ok_or_else(|| format!("missing origin: {field}").into())
}

pub(super) fn assert_marker(meta: &Value, field: &str, encrypted: bool) -> TestResult {
    let origin = origin(meta, field)?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "{field}"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(true)),
        "{field}"
    );
    Ok(())
}

pub(super) fn uses<'a>(meta: &'a Value, field: &str) -> TestResult<Vec<&'a Value>> {
    let refs = origin(meta, field)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source references")?;
    let uses = meta
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    refs.iter()
        .map(|id| {
            uses.iter()
                .find(|site| site.get("id") == Some(id))
                .ok_or_else(|| format!("unresolved use for {field}: {id}").into())
        })
        .collect()
}

pub(super) fn source_path<'a>(meta: &'a Value, site: &Value) -> TestResult<&'a str> {
    physical_source(meta, site)?
        .pointer("/path/value")
        .and_then(Value::as_str)
        .ok_or_else(|| "source path".into())
}

pub(super) fn physical_source<'a>(meta: &'a Value, site: &Value) -> TestResult<&'a Value> {
    let id = site.get("source").ok_or("physical source reference")?;
    meta.pointer("/provenance/sources")
        .and_then(Value::as_array)
        .and_then(|sources| sources.iter().find(|source| source.get("id") == Some(id)))
        .ok_or_else(|| format!("unresolved physical source: {id}").into())
}
