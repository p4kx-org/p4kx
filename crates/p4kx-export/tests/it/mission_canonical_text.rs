// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::mission_template_graph::{self as source, Property};

/// Checks an existing lookup through its actual occurrence and canonical protected text row.
pub(crate) fn key(
    property: &Property<'_>,
    catalog: &Value,
    node: &Value,
    original: &str,
    interpretation: &str,
    expected: Option<&str>,
) -> TestResult {
    token(node, original)?;
    let lookup = lookup(property, node)?;
    assert_eq!(
        lookup.get("interpretation"),
        Some(&json!(interpretation)),
        "normalization remains separate from original token preservation"
    );
    let cell = lookup
        .get("text")
        .ok_or("independently sourced lookup result")?;
    if let Some(expected) = expected {
        definition(catalog, cell, expected)?;
    } else {
        absent(cell);
    }
    Ok(())
}

fn token(node: &Value, original: &str) -> TestResult {
    assert_eq!(
        source::text(node)?,
        original,
        "the exact original token exists before its contextual lookup is tested"
    );
    let payload = raw::present(node.get("value").ok_or("actual source text cell")?)?;
    assert_eq!(
        payload.pointer("/value/type"),
        Some(&json!("gameText")),
        "the original key keeps its protected text domain"
    );
    Ok(())
}

fn lookup<'catalog>(property: &Property<'catalog>, node: &Value) -> TestResult<&'catalog Value> {
    let reference = property
        .owner
        .reference(crate::flow_owner_graph::ordinal(node)?)?;
    let rows = raw::array(property.projection, "localizations")?;
    let selected: Vec<_> = rows
        .iter()
        .filter(|row| row.get("source") == Some(&reference))
        .collect();
    assert_eq!(
        selected.len(),
        1,
        "the established localized presentation keeps one lookup for this actual source occurrence"
    );
    selected
        .first()
        .copied()
        .ok_or_else(|| "actual canonical text lookup".into())
}

fn definition(catalog: &Value, cell: &Value, expected: &str) -> TestResult {
    let id = raw::present(cell)?;
    let rows = catalog
        .pointer("/texts/gameTexts")
        .and_then(Value::as_array)
        .ok_or("canonical protected text definitions")?;
    let matching: Vec<_> = rows
        .iter()
        .filter(|row| row.pointer("/id/value") == Some(id))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "the contextual GameTextRef resolves to one canonical protected text"
    );
    let row = *matching.first().ok_or("canonical protected key")?;
    assert_eq!(
        raw::present(row.get("key").ok_or("original canonical key")?)?,
        &json!(expected),
        "the established presentation key retains its exact meaning"
    );
    Ok(())
}

fn absent(cell: &Value) {
    assert_eq!(
        cell.get("state"),
        Some(&json!("absent")),
        "empty, placeholder and non-key inputs do not invent a presentation key"
    );
    assert_eq!(
        cell.get("value"),
        Some(&Value::Null),
        "the canonical absent lookup carries no fabricated key"
    );
}

pub(crate) fn single(node: &Value, expected: f32) -> TestResult {
    let number = source::scalar(node, "number")?;
    assert_eq!(
        number.get("type"),
        Some(&json!("f32")),
        "the exact source Single type is retained"
    );
    assert_eq!(
        number.get("finite"),
        Some(&json!(expected)),
        "the original finite fraction is exact"
    );
    assert_eq!(
        number.get("rawBits"),
        Some(&json!(format!("{:08x}", expected.to_bits()))),
        "the original IEEE Single bits are retained independently of display rounding"
    );
    Ok(())
}
