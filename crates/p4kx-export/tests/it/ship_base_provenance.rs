// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::time::UNIX_EPOCH;

use p4kx_export::{ShipBaseOptions, build_ship_base};
use p4kx_extraction::Archive;
use serde_json::Value;

use crate::ship_base_fixture::write_nested_vehicle_provenance_archive;

use super::TestResult;

#[test]
fn standalone_formats_keep_legacy_values_and_independent_exact_origins() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = write_nested_vehicle_provenance_archive(directory.path())?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let container = archive
        .entries()
        .first()
        .ok_or("synthetic containing Entry")?;
    let inner = archive.open_container(container)?;
    let built = build_ship_base(&inner, &ShipBaseOptions::new(UNIX_EPOCH))?;
    let mut bytes = Vec::new();
    built.write_json(&mut bytes)?;
    let json: Value = serde_json::from_slice(&bytes)?;
    let rows = json
        .get("ships")
        .and_then(Value::as_array)
        .ok_or("standalone rows")?;
    let whole = json
        .get("provenance")
        .ok_or("JSON aggregate evidence closure")?;
    bytes.clear();
    built.write_ndjson(&mut bytes)?;
    let lines = std::str::from_utf8(&bytes)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    assert_eq!(
        rows.len(),
        lines.len(),
        "preserve one NDJSON line per written Ship"
    );
    for ((expected, mut line), legacy) in rows.iter().zip(lines).zip(built.ships()) {
        let object = line.as_object_mut().ok_or("standalone line object")?;
        let local = object
            .remove("provenance")
            .ok_or("NDJSON evidence closure")?;
        assert_eq!(
            &line, expected,
            "both writers use identical flat vehicle rows"
        );
        for (field, value) in serde_json::to_value(legacy)?
            .as_object()
            .ok_or("legacy Ship")?
        {
            assert_eq!(
                line.get(field),
                Some(value),
                "legacy address and value: {field}"
            );
        }
        verify_line(&line, &local, whole)?;
    }
    verify_mixed(rows)
}

fn verify_mixed(rows: &[Value]) -> TestResult {
    let first = rows
        .iter()
        .find(|row| row.get("className").and_then(Value::as_str) == Some("ACME_Alpha"))
        .ok_or("vehicle using encrypted XML")?;
    assert_eq!(
        first.pointer("/fieldOrigins/~1guid/containsEncryptedSource"),
        Some(&Value::Bool(false)),
        "the original DataForge identity remains clear"
    );
    assert_eq!(
        first.pointer("/fieldOrigins/~1mass~1kg/containsEncryptedSource"),
        Some(&Value::Bool(true)),
        "the selected encrypted vehicle XML contributes to the original numerical value"
    );
    Ok(())
}

fn verify_line(row: &Value, local: &Value, whole: &Value) -> TestResult {
    let local_uses = by_id(local, "uses")?;
    let mut refs = BTreeSet::new();
    let fields = row
        .get("fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("field origins")?;
    for (address, origin) in fields {
        assert!(
            row.pointer(address).is_some(),
            "origin addresses an existing value: {address}"
        );
        refs.extend(references(origin)?);
    }
    refs.extend(references(
        row.get("playability").ok_or("sourced playability")?,
    )?);
    assert!(
        refs.iter().copied().eq(local_uses.keys().copied()),
        "selected uses match row references exactly"
    );
    let full_uses = by_id(whole, "uses")?;
    for (id, use_row) in &local_uses {
        assert_eq!(
            full_uses.get(id),
            Some(use_row),
            "the line keeps original observations"
        );
    }
    let full_sources = by_id(whole, "sources")?;
    let expected = reached_sources(&local_uses, &full_sources)?;
    let local_sources = by_id(local, "sources")?;
    assert_eq!(
        local_sources, expected,
        "line retains only original sources and ancestor closure"
    );
    if row.get("className").and_then(Value::as_str) == Some("ACME_Ground") {
        assert!(
            local_sources.len() < full_sources.len(),
            "the physics-only row omits unrelated vehicle XML"
        );
        assert!(
            local_sources.len() >= 2,
            "the source closure includes the enclosing original Entry"
        );
    }
    Ok(())
}

fn references(origin: &Value) -> Result<Vec<&str>, Box<dyn std::error::Error>> {
    origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("value origin references")?
        .iter()
        .map(|value| value.as_str().ok_or_else(|| "reference ID".into()))
        .collect()
}

fn by_id<'wire>(
    document: &'wire Value,
    field: &str,
) -> Result<BTreeMap<&'wire str, &'wire Value>, Box<dyn std::error::Error>> {
    document
        .get(field)
        .and_then(Value::as_array)
        .ok_or("provenance rows")?
        .iter()
        .map(|row| Ok((row.get("id").and_then(Value::as_str).ok_or("row ID")?, row)))
        .collect()
}

fn reached_sources<'wire>(
    uses: &BTreeMap<&str, &'wire Value>,
    available: &BTreeMap<&'wire str, &'wire Value>,
) -> Result<BTreeMap<&'wire str, &'wire Value>, Box<dyn std::error::Error>> {
    let mut pending = uses
        .values()
        .copied()
        .map(|row| {
            row.get("source")
                .and_then(Value::as_str)
                .ok_or("original use source")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut reached = BTreeMap::new();
    while let Some(id) = pending.pop() {
        let source = available
            .get(id)
            .copied()
            .ok_or("resolvable original source")?;
        if reached.insert(id, source).is_none() {
            let ancestors = source
                .get("ancestors")
                .and_then(Value::as_array)
                .ok_or("original ancestor array")?;
            for ancestor in ancestors {
                pending.push(ancestor.as_str().ok_or("original ancestor ID")?);
            }
        }
    }
    Ok(reached)
}
