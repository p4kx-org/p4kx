// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Map, Value};

use crate::shader_payload::ShaderPayload;
use crate::shader_projection::{
    CACHE_NAMESPACE, DECLARATION_NAMESPACE, DEFINITION_NAMESPACE, FAILURE_NAMESPACE,
    PROPERTY_NAMESPACE,
};
use crate::{MediaError, schema_merge};

/// Returns the precise schema of the built-in shader payload union.
#[must_use]
pub fn shader_payload_schema() -> Value {
    schemars::schema_for!(ShaderPayload).to_value()
}

/// Returns the common record schema with conditions limited to built-in shader namespaces.
/// Custom provider namespaces retain the general `MediaRecord` data contract.
///
/// # Errors
/// Returns a contract error if generated definition names collide or schema shapes change.
pub fn shader_record_schema() -> Result<Value, MediaError> {
    bind_record_schema(schemars::schema_for!(crate::MediaRecord).to_value())
}

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    schema_merge::insert_payload(&mut base, "ShaderPayload", shader_payload_schema())?;
    let root = base
        .as_object_mut()
        .ok_or_else(|| contract("media record schema is not an object"))?;
    let mut bindings = vec![
        binding(DEFINITION_NAMESPACE, "definition", Some("complete")),
        binding(PROPERTY_NAMESPACE, "property", Some("complete")),
        binding(CACHE_NAMESPACE, "compiled-cache", Some("complete")),
        binding(FAILURE_NAMESPACE, "definition-failure", Some("failed")),
        binding(DECLARATION_NAMESPACE, "declaration", None),
    ];
    bindings.push(mask_status_binding());
    let constraints = root
        .entry("allOf")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| contract("media record allOf is not an array"))?;
    constraints.extend(bindings);
    Ok(base)
}

fn binding(namespace: &str, record_type: &str, status: Option<&str>) -> Value {
    let data = serde_json::json!({
        "allOf": [
            {"$ref":"#/$defs/ShaderPayload"},
            {"properties":{"record_type":{"const":record_type}},"required":["record_type"]}
        ]
    });
    let mut properties = Map::new();
    properties.insert("data".into(), data);
    if let Some(status) = status {
        properties.insert("status".into(), serde_json::json!({"const":status}));
        properties.insert("issues".into(), issue_schema(status));
    }
    serde_json::json!({
        "if": owned_namespace(namespace),
        "then": {"properties":properties,"required":["data"]}
    })
}

fn owned_namespace(namespace: &str) -> Value {
    serde_json::json!({
        "properties": {
            "kind":{"const":"shader"},
            "subresource":{"properties":{"namespace":{"const":namespace}},"required":["namespace"]}
        }, "required":["kind","subresource"]
    })
}

fn mask_status_binding() -> Value {
    serde_json::json!({
        "if":owned_namespace(DECLARATION_NAMESPACE),
        "then": {
            "if": {
                "properties":{"data":{
                    "properties":{"payload":{
                        "properties":{"meaning":{
                            "properties":{
                                "role":{"const":"mask"},
                                "interpretation":{"properties":{"state":{"enum":["malformed","unsupported-reader-variant"]}},"required":["state"]}
                            },"required":["role","interpretation"]
                        }},"required":["meaning"]
                    }},"required":["payload"]
                }},"required":["data"]
            },
            "then":{"properties":{"status":{"const":"partial"},"issues":issue_schema("partial")}},
            "else":{"properties":{"status":{"const":"complete"},"issues":issue_schema("complete")}}
        }
    })
}

fn issue_schema(status: &str) -> Value {
    if status == "complete" {
        serde_json::json!({"items":{"properties":{"severity":{"const":"info"}},"required":["severity"]}})
    } else {
        let severities = if status == "failed" {
            vec!["error"]
        } else {
            vec!["warning", "error"]
        };
        serde_json::json!({"minItems":1,"contains":{"properties":{"severity":{"enum":severities}},"required":["severity"]}})
    }
}

fn contract(detail: &str) -> MediaError {
    MediaError::Contract(detail.into())
}
