// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::material_payload::MaterialPayload;
use crate::material_projection::{FAILURE_NAMESPACE, LIBRARY_NAMESPACE};
use crate::{MediaError, schema_merge};

/// Returns the precise built-in material-library and failure payload schema.
#[must_use]
pub fn material_payload_schema() -> Value {
    schemars::schema_for!(MaterialPayload<'static>).to_value()
}

/// Returns the common record schema constrained only for owned material namespaces.
///
/// # Errors
/// Returns incompatible generated definitions or an invalid common schema shape.
pub fn material_record_schema() -> Result<Value, MediaError> {
    bind_record_schema(crate::record_schema())
}

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    schema_merge::insert_payload(&mut base, "MaterialPayload", material_payload_schema())?;
    let root = base
        .as_object_mut()
        .ok_or_else(|| MediaError::Contract("material record schema is not an object".into()))?;
    let constraints = root
        .entry("allOf")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| MediaError::Contract("media record allOf is not an array".into()))?;
    for (namespace, record_type) in [
        (LIBRARY_NAMESPACE, "library"),
        (FAILURE_NAMESPACE, "failure"),
    ] {
        constraints.push(serde_json::json!({
            "if": {"properties":{"kind":{"const":"material"},"subresource":{"properties":{"namespace":{"const":namespace}},"required":["namespace"]}},"required":["kind","subresource"]},
            "then": {"properties":{"data":{"allOf":[{"$ref":"#/$defs/MaterialPayload"},{"properties":{"record_type":{"const":record_type}},"required":["record_type"]}]}},"required":["data"]}
        }));
    }
    constraints.push(serde_json::json!({
        "if":{"properties":{"kind":{"const":"material"},"subresource":{"properties":{"namespace":{"const":FAILURE_NAMESPACE}},"required":["namespace"]}},"required":["kind","subresource"]},
        "then":{"properties":{"status":{"const":"failed"},"issues":{"contains":{"properties":{"severity":{"const":"error"}},"required":["severity"]}}}}
    }));
    constraints.push(serde_json::json!({
        "if":{"properties":{"kind":{"const":"material"},"subresource":{"properties":{"namespace":{"const":LIBRARY_NAMESPACE}},"required":["namespace"]}},"required":["kind","subresource"]},
        "then":{"if":{"properties":{"data":{"properties":{"payload":{"properties":{"projection_complete":{"const":true}},"required":["projection_complete"]}},"required":["payload"]}},"required":["data"]},
            "then":{"properties":{"status":{"const":"complete"},"issues":{"items":{"properties":{"severity":{"const":"info"}},"required":["severity"]}}}},
            "else":{"properties":{"status":{"const":"partial"},"issues":{"contains":{"properties":{"severity":{"enum":["warning","error"]}},"required":["severity"]}}}}}
    }));
    constraints.push(serde_json::json!({
        "if":{"allOf":[
            {"properties":{"kind":{"const":"material"},"subresource":{"properties":{"namespace":{"const":LIBRARY_NAMESPACE}},"required":["namespace"]}},"required":["kind","subresource"]},
            {"properties":{"data":{"properties":{"payload":{"anyOf":[
                {"properties":{"authored":{"properties":{"losses":{"minItems":1}},"required":["losses"]}},"required":["authored"]},
                {"properties":{"authored":{"properties":{"root":{"type":"null"}},"required":["root"]}},"required":["authored"]},
                {"properties":{"materials":{"contains":{"properties":{"authored":{"properties":{"truncated":{"minItems":1}},"required":["truncated"]}},"required":["authored"]}}},"required":["materials"]}
            ]}},"required":["payload"]}},"required":["data"]}
        ]},
        "then":{"properties":{"status":{"const":"partial"},"data":{"properties":{"payload":{"properties":{"projection_complete":{"const":false}}}}}}}
    }));
    Ok(base)
}
