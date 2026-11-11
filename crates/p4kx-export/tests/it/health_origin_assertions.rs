// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_fixture::TestResult;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(crate) fn at<'a>(value: &'a Value, path: &str) -> TestResult<&'a Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing actual product field {path}").into())
}
pub(crate) fn rows<'a>(value: &'a Value, path: &str) -> TestResult<&'a [Value]> {
    Ok(at(value, path)?.as_array().ok_or("actual product rows")?)
}
pub(crate) fn cell<'a>(row: &'a Value, field: &str, state: &str) -> TestResult<&'a Value> {
    let value = row
        .get(field)
        .ok_or_else(|| format!("missing actual health identity {field}"))?;
    assert!(
        value.is_object(),
        "health identity {field} needs its own immediate sourced carrier: {value}"
    );
    let names: BTreeSet<_> = value
        .as_object()
        .ok_or("sourced identity object")?
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        names,
        BTreeSet::from([
            "state",
            "value",
            "containsEncryptedSource",
            "sourceUsesRefs",
            "sourceSetComplete"
        ]),
        "{field} uses the shared five-field contract"
    );
    assert_eq!(
        value.get("state"),
        Some(&json!(state)),
        "the actual source state of {field} survives"
    );
    if state != "present" {
        assert_eq!(
            value.get("value"),
            Some(&Value::Null),
            "non-present {field} retains explicit null"
        );
    }
    Ok(value)
}
pub(crate) fn present<'a>(
    row: &'a Value,
    field: &str,
    expected: &Value,
    marker: Option<bool>,
) -> TestResult<&'a Value> {
    let value = cell(row, field, "present")?;
    assert_eq!(
        value.get("value"),
        Some(expected),
        "{field} retains the actual original value"
    );
    assert_eq!(
        value.get("containsEncryptedSource"),
        Some(&json!(marker)),
        "{field} has its actual independent origin"
    );
    assert_eq!(
        value.get("sourceSetComplete"),
        Some(&json!(true)),
        "the complete original identity {field} is captured"
    );
    Ok(value)
}
pub(crate) fn present_text<'row>(
    catalog: &Value,
    row: &'row Value,
    field: &str,
    expected: &Value,
    marker: Option<bool>,
) -> TestResult<&'row Value> {
    let cell = cell(row, field, "present")?;
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&json!(marker)),
        "the semantic occurrence retains its actual independent origin"
    );
    assert_eq!(
        cell.get("sourceSetComplete"),
        Some(&json!(true)),
        "the retained original text occurrence is complete"
    );
    assert_eq!(
        text_value(at(catalog, "/exportHealth")?, cell)?,
        expected,
        "the original complete text survives pooling"
    );
    Ok(cell)
}

pub(crate) fn example_text<'catalog>(
    catalog: &'catalog Value,
    example: &Value,
    field: &str,
) -> TestResult<Option<&'catalog Value>> {
    let cell = at(example, &format!("/{field}"))?;
    if cell.get("state").and_then(Value::as_str) != Some("present") {
        return Ok(None);
    }
    text_value(at(catalog, "/exportHealth")?, cell).map(Some)
}

pub(crate) fn text_value<'document>(
    health: &'document Value,
    cell: &Value,
) -> TestResult<&'document Value> {
    assert_eq!(
        cell.get("state"),
        Some(&json!("present")),
        "the semantic text is present"
    );
    let reference = cell
        .get("value")
        .and_then(Value::as_object)
        .ok_or("actual text reference")?;
    assert_eq!(
        reference.len(),
        1,
        "the typed text reference has only its structural index"
    );
    let raw = reference
        .get("atomIndex")
        .and_then(Value::as_str)
        .ok_or("actual atom index")?;
    let index: usize = raw.parse()?;
    assert_eq!(
        index.to_string(),
        raw,
        "the document-local index is canonical decimal"
    );
    let atom = rows(health, "/metadata/texts")?
        .get(index)
        .ok_or("the occurrence references an actual atom of this same report")?;
    assert_eq!(
        atom.get("state"),
        Some(&json!("present")),
        "the referenced text exists"
    );
    atom.get("value")
        .ok_or_else(|| "the referenced atom retains its semantic text".into())
}

pub(crate) fn uses<'a>(catalog: &'a Value, carrier: &Value) -> TestResult<Vec<&'a Value>> {
    crate::canonical_header_fixture::use_rows(catalog, carrier)
}
pub(crate) fn source<'a>(catalog: &'a Value, used: &Value) -> TestResult<&'a Value> {
    let id = used.get("source").ok_or("actual source id")?;
    let sources = rows(catalog, "/provenance/sources")?;
    let matching: Vec<_> = sources
        .iter()
        .filter(|source| source.get("id") == Some(id))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "every actual use has one original physical source"
    );
    matching
        .first()
        .copied()
        .ok_or_else(|| "closed physical source".into())
}
pub(crate) fn header(
    catalog: &Value,
    carrier: &Value,
    index: u32,
    field: &str,
    role: &str,
    input_kind: &str,
) -> TestResult {
    let used = uses(catalog, carrier)?;
    assert_eq!(
        used.len(),
        1,
        "an identity header consumes exactly its own field"
    );
    let used = used.first().ok_or("original header use")?;
    assert_eq!(
        at(used, "/location/type")?,
        &json!("subresource"),
        "header and body coordinates stay distinct"
    );
    assert_eq!(
        at(used, "/location/namespace")?,
        &json!("dataforgeRecordHeader"),
        "the original header namespace is retained"
    );
    assert_eq!(
        at(used, "/location/indices")?,
        &json!([index.to_string()]),
        "the original physical owner is retained"
    );
    assert_eq!(
        at(used, "/location/field")?,
        &json!(field),
        "only this original header field contributes"
    );
    assert_eq!(
        used.get("role"),
        Some(&json!(role)),
        "the actual metadata operation is retained"
    );
    assert_eq!(
        used.get("scope"),
        Some(&json!("payload")),
        "Record headers are DCB payload facts"
    );
    assert_eq!(
        source(catalog, used)?.get("kind"),
        Some(&json!(input_kind)),
        "no supplied enrichment Archive impersonates a standalone DCB"
    );
    Ok(())
}
pub(crate) fn independent(carrier: &Value) -> TestResult {
    assert_eq!(
        carrier.get("containsEncryptedSource"),
        Some(&json!(false)),
        "a fixed control has independent plain provenance"
    );
    assert_eq!(
        carrier.get("sourceSetComplete"),
        Some(&json!(true)),
        "an explicit independent value is complete"
    );
    assert!(
        at(carrier, "/sourceUsesRefs")?
            .as_array()
            .ok_or("independent uses")?
            .is_empty(),
        "an independent value has no fabricated source use"
    );
    Ok(())
}
pub(crate) fn closure(catalog: &Value) -> TestResult {
    crate::commodity_v3_source_inventory::reference_closure(catalog)
}

#[path = "catalog_declaration_fixture.rs"]
pub(crate) mod declaration_fixture;
