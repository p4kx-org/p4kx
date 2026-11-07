// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::{
    MediaError, archive_extras_read, archive_layout_projection, archive_metadata_diagnostic,
    archive_records_read, archive_retention, archive_sections_read, archive_tables_read,
};

pub(crate) fn bind(mut base: Value) -> Result<Value, MediaError> {
    let rules = [
        (
            archive_layout_projection::LAYOUT_NAMESPACE,
            json!({"anyOf": [different("declarationState", "available"),
                {"properties":{"layout":{"const":"unknown"}}, "required":["layout"]}
            ]}),
        ),
        (
            archive_sections_read::SECTION_NAMESPACE,
            json!({"anyOf": [
                {"properties":{"presence":{"enum":["invalid-declaration", "unavailable-layout"]}}, "required":["presence"]},
                {"properties":{"region":{"type":"object", "properties":{"state":{"not":{"const":"complete"}}}, "required":["state"]}}, "required":["region"]}
            ]}),
        ),
        (
            archive_records_read::RECORD_NAMESPACE,
            json!({"properties":{"format":{"const":"unknown"}}, "required":["format"]}),
        ),
        (
            archive_extras_read::EXTRA_NAMESPACE,
            different("state", "complete"),
        ),
        (
            archive_tables_read::INSTALL_NAMESPACE,
            json!({"anyOf":[
                {"properties":{"reservedState":{"const":"nonzero"}}, "required":["reservedState"]},
                {"properties":{"bound":{"const":"exceeds-stored-size"}}, "required":["bound"]}
            ]}),
        ),
        (
            archive_tables_read::FREE_NAMESPACE,
            json!({"properties":{"state":{"not":{"enum":["within-source","zero-size"]}}},"required":["state"]}),
        ),
        (
            archive_metadata_diagnostic::DIAGNOSTIC_NAMESPACE,
            Value::Bool(true),
        ),
        (archive_retention::RAW_NAMESPACE, Value::Bool(false)),
    ];
    let root = base
        .as_object_mut()
        .ok_or_else(|| MediaError::Contract("media record schema is not an object".into()))?;
    let constraints = root
        .get_mut("allOf")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MediaError::Contract("media record allOf is not an array".into()))?;
    constraints.extend(
        rules
            .into_iter()
            .map(|(namespace, invalid)| binding(namespace, &invalid)),
    );
    Ok(base)
}

fn different(field: &str, value: &str) -> Value {
    json!({"properties": {(field): {"not": {"const": value}}}, "required": [field]})
}

fn binding(namespace: &str, invalid: &Value) -> Value {
    json!({
        "if": {"properties": {"kind":{"const":"archive"}, "subresource":{"properties":{"namespace":{"const":namespace}}, "required":["namespace"]}}, "required":["kind","subresource"]},
        "then": {
            "if": {"properties":{"data":invalid}, "required":["data"]},
            "then": {"properties": {
                "status":{"const":"partial"},
                "issues":{"minItems":1,"contains":{"properties":{"severity":{"enum":["warning","error"]}}, "required":["severity"]}}
            }, "required":["status","issues"]},
            "else": {"properties": {
                "status":{"const":"complete"},
                "issues":{"items":{"properties":{"severity":{"const":"info"}}, "required":["severity"]}}
            }, "required":["status","issues"]}
        }
    })
}
