// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::header::{NAMESPACE, TextureHeaderPayload};
use crate::{MediaError, schema_merge};

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    const PAYLOAD: &str = "TextureHeaderPayload";
    schema_merge::insert_payload(
        &mut base,
        PAYLOAD,
        schema_merge::serialized_payload::<TextureHeaderPayload>(),
    )?;
    let branches = base
        .get_mut("allOf")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MediaError::Contract("media schema has no built-in constraints".into()))?;
    branches.push(json!({
        "if": {
            "properties": {
                "kind": {"const":"texture"},
                "subresource": {"properties":{"namespace":{"const":NAMESPACE}}, "required":["namespace"]}
            },
            "required":["kind","subresource"]
        },
        "then": {
            "properties": {
                "data":{"$ref":format!("#/$defs/{PAYLOAD}")},
                "subresource":{"properties":{"indices":{"maxItems":0}}}
            },
            "allOf": [
                {
                    "if":{"properties":{"data":{"properties":{"record_type":{"const":"header"}}}}},
                    "then":{"properties":{"status":{"const":"complete"}}}
                },
                {
                    "if":{"properties":{"data":{"properties":{"record_type":{"const":"invalid-header"}}}}},
                    "then":{"properties":{"status":{"const":"partial"}}}
                }
            ]
        }
    }));
    Ok(base)
}
