// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::support::TestResult;

pub(super) fn assert_all_addresses(response: &Value) -> TestResult {
    let fields = response
        .pointer("/structuredContent/fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("field origins")?;
    for pointer in fields.keys() {
        assert!(
            response.pointer(pointer).is_some(),
            "nonexistent field {pointer}"
        );
    }
    Ok(())
}

pub(super) fn marker(origin: &Value, encrypted: bool) {
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(encrypted),
        "field marker"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "complete sources"
    );
}

pub(super) fn assert_site(
    reply: &Value,
    output: &str,
    namespace: &str,
    field: &str,
    indices: &[&str],
) -> TestResult {
    let coordinates = json!(indices);
    assert!(
        uses(reply, output)?.iter().any(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && site.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && site.pointer("/location/indices") == Some(&coordinates)
        }),
        "{output} must point to {namespace}:{field} {indices:?}"
    );
    Ok(())
}

pub(super) fn field<'a>(reply: &'a Value, output: &str) -> TestResult<&'a Value> {
    reply
        .get("fieldOrigins")
        .and_then(|fields| fields.get(format!("/structuredContent/{output}")))
        .ok_or_else(|| format!("missing field origin for {output}").into())
}

pub(super) fn uses<'a>(reply: &'a Value, output: &str) -> TestResult<Vec<&'a Value>> {
    let available = inventory_uses(reply)?;
    field(reply, output)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("field uses")?
        .iter()
        .map(|reference| {
            available
                .iter()
                .find(|site| site.get("id") == Some(reference))
                .ok_or_else(|| format!("unresolved field use for {output}").into())
        })
        .collect()
}

pub(super) fn inventory_uses(reply: &Value) -> TestResult<&[Value]> {
    reply
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "registered source uses".into())
}
