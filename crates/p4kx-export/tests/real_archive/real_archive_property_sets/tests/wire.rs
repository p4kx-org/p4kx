// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{ExportDiagnosticCode, ExportDiagnosticStage, ExportSourceRole, Kind};
use p4kx_extraction::Record;
use serde_json::Value;

use super::super::TestResult;
use super::privacy;

/// A negative wire value whose immediate inputs are explicitly unknown.
pub(super) fn present(value: Value) -> Value {
    Value::Object(
        [
            ("state".to_owned(), Value::String("present".to_owned())),
            ("value".to_owned(), value),
            ("containsEncryptedSource".to_owned(), Value::Null),
            ("sourceUsesRefs".to_owned(), Value::Array(Vec::new())),
            ("sourceSetComplete".to_owned(), Value::Bool(false)),
        ]
        .into_iter()
        .collect(),
    )
}
pub(super) fn absent() -> Value {
    serde_json::json!({
        "state": "absent", "value": null,
        "containsEncryptedSource": false, "sourceUsesRefs": [], "sourceSetComplete": true,
    })
}

fn atom(document: &mut Value, text: &str) -> TestResult<Value> {
    let texts = document
        .pointer_mut("/exportHealth/metadata/texts")
        .and_then(Value::as_array_mut)
        .ok_or("same-document text storage absent")?;
    let index = texts
        .iter()
        .position(|cell| cell.get("value").and_then(Value::as_str) == Some(text))
        .unwrap_or_else(|| {
            let index = texts.len();
            texts.push(present(Value::String(text.to_owned())));
            index
        });
    Ok(serde_json::json!({"atomIndex": u64::try_from(index)?.to_string()}))
}

pub(super) fn failure(
    document: &mut Value,
    record: &Record<'_>,
    kind: Kind,
    role: ExportSourceRole,
    stage: ExportDiagnosticStage,
    code: ExportDiagnosticCode,
) -> TestResult<Value> {
    let path = atom(document, record.record_path()?)?;
    let source_type = atom(document, record.type_name()?)?;
    Ok(serde_json::json!({
        "kind": kind, "role": role, "stage": stage, "code": code,
        "recordIndex": present(serde_json::json!(record.index())),
        "guid": present(serde_json::json!(record.guid().to_string())),
        "recordPath": present(path), "sourceType": present(source_type),
    }))
}

pub(super) fn projection_diagnostic(failed: &Value) -> TestResult<Value> {
    let field = |name: &str| {
        failed
            .get(name)
            .cloned()
            .ok_or("failed mutation field absent")
    };
    Ok(serde_json::json!({
        "kind": field("kind")?, "stage": field("stage")?, "code": field("code")?,
        "severity": "error", "sourceRole": field("role")?, "warningCode": null,
        "count": 1, "countUnit": "records",
        "examples": [{
            "recordIndex": field("recordIndex")?, "recordGuid": field("guid")?,
            "recordPath": field("recordPath")?, "field": null, "sourceType": null, "targetGuid": null,
        }],
    }))
}

pub(super) fn diagnostic(document: &mut Value, index: u32, prefix: &str) -> TestResult<Value> {
    let path = atom(document, privacy::SENTINEL)?;
    Ok(serde_json::json!({
        "kind": Kind::MissionPropertySets, "stage": ExportDiagnosticStage::DerivedProjection,
        "code": ExportDiagnosticCode::UnsupportedValue, "severity": "error",
        "sourceRole": ExportSourceRole::DerivedInput, "warningCode": privacy::SENTINEL,
        "count": 1, "countUnit": "records",
        "examples": [{
            "recordIndex": present(serde_json::json!(index)),
            "recordGuid": present(serde_json::json!(privacy::SENTINEL)), "recordPath": present(path),
            "field": privacy::SENTINEL, "sourceType": format!("{prefix}{}", privacy::SENTINEL),
            "targetGuid": privacy::SENTINEL,
        }],
    }))
}

pub(super) fn diagnostics(document: &mut Value, values: Vec<Value>) -> TestResult {
    let target = document
        .pointer_mut("/exportHealth/diagnostics")
        .ok_or("diagnostics absent")?;
    *target = Value::Array(values);
    Ok(())
}

pub(super) fn push_failure(document: &mut Value, failed: Value) -> TestResult {
    document
        .pointer_mut("/exportHealth/failedRecords")
        .and_then(Value::as_array_mut)
        .ok_or("actual failed Records absent")?
        .push(failed);
    Ok(())
}
