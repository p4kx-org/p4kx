// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::health_origin_assertions as proof;

pub(super) fn require_use(catalog: &Value, carrier: &Value, field: &str, role: &str) -> TestResult {
    assert!(
        proof::uses(catalog, carrier)?.iter().any(|used| used
            .pointer("/location/namespace")
            .and_then(Value::as_str)
            == Some("dataforgeValue")
            && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
            && used.pointer("/location/indices/0").and_then(Value::as_str) == Some("0")
            && used["role"] == role),
        "the actual physical owner contributes {field} as {role}"
    );
    Ok(())
}

pub(super) fn check_diagnostic(catalog: &Value, code: &str, path: &str) -> TestResult {
    let diagnostic = proof::rows(catalog, "/exportHealth/diagnostics")?
        .iter()
        .find(|row| row["code"] == code && row["kind"] == "items")
        .ok_or("typed Items field diagnostic")?;
    for example in proof::rows(diagnostic, "/examples")? {
        if proof::example_text(catalog, example, "field")? == Some(&json!(path)) {
            return Ok(());
        }
    }
    Err(format!("missing original diagnostic field {path}").into())
}
