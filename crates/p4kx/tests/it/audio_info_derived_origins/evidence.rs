// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::TestResult;

pub(super) fn assert_site(
    document: &Value,
    output: &str,
    namespace: &str,
    indices: &[u64],
    field: &str,
    role: &str,
) -> TestResult {
    let location = location(namespace, indices, field);
    let uses = contributors(document, output)?;
    assert!(
        uses.iter().any(|used| {
            used.get("location") == Some(&location)
                && used.get("role").and_then(Value::as_str) == Some(role)
        }),
        "{output} must use {location} as {role}, got {uses:?}"
    );
    Ok(())
}

pub(super) fn exclude_site(
    document: &Value,
    namespace: &str,
    indices: &[u64],
    field: &str,
) -> TestResult {
    let location = location(namespace, indices, field);
    assert!(
        observations(document)?
            .iter()
            .all(|used| used.get("location") != Some(&location)),
        "{location} was not used"
    );
    Ok(())
}

pub(super) fn exclude_chunks(document: &Value, bodies: &[u64]) -> TestResult {
    let observations = observations(document)?;
    for body in bodies {
        let coordinate = body.to_string();
        assert!(
            observations.iter().all(|used| {
                !matches!(
                    used.pointer("/location/namespace").and_then(Value::as_str),
                    Some("wemChunk" | "wemLoop" | "wemLoopPrefix")
                ) || used.pointer("/location/indices/0").and_then(Value::as_str)
                    != Some(coordinate.as_str())
            }),
            "unselected chunk body at {body} has no retained field or loop evidence"
        );
    }
    Ok(())
}

fn contributors<'a>(document: &'a Value, output: &str) -> TestResult<Vec<&'a Value>> {
    let origin = document
        .get("fieldOrigins")
        .and_then(|fields| fields.get(output))
        .ok_or_else(|| format!("immediate Audio Info origin {output}"))?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&Value::Null),
        "{output}"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(true)),
        "{output}"
    );
    let ids = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source-use IDs")?;
    let observations = observations(document)?;
    let source = document
        .pointer("/provenance/sources/0/id")
        .ok_or("standalone input ID")?;
    ids.iter()
        .map(|id| {
            let used = observations
                .iter()
                .find(|used| used.get("id") == Some(id))
                .ok_or_else(|| format!("unknown source-use ID {id}"))?;
            assert_eq!(used.get("source"), Some(source), "{output}");
            assert_eq!(
                used.get("scope").and_then(Value::as_str),
                Some("payload"),
                "{output}"
            );
            Ok(used)
        })
        .collect()
}

fn observations(document: &Value) -> TestResult<&[Value]> {
    Ok(document
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("Audio Info source uses")?)
}

fn location(namespace: &str, indices: &[u64], field: &str) -> Value {
    json!({
        "type": "subresource",
        "namespace": namespace,
        "indices": indices.iter().map(u64::to_string).collect::<Vec<_>>(),
        "field": field,
    })
}
