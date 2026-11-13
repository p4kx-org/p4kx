// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::payload::{FRAME, NODE, PhysicsPayload, SLOT, TABLE, TRIANGLE, VERTEX};
use crate::{MediaError, schema_merge};

pub(in crate::provider::model_source) fn bind(base: &mut Value) -> Result<(), MediaError> {
    const PAYLOAD: &str = "PhysicsPayload";
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<PhysicsPayload>(),
    )?;
    let branches = base
        .get_mut("allOf")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MediaError::Contract("media schema has no built-in constraints".into()))?;
    for (namespace, record_type, count) in [
        (TABLE, "table", 1),
        (SLOT, "slot", 2),
        (VERTEX, "vertex", 3),
        (NODE, "node", 2),
        (FRAME, "frame", 2),
        (TRIANGLE, "triangle", 3),
    ] {
        branches.push(json!({
            "if":{"properties":{
                "kind":{"const":"model"},
                "subresource":{"properties":{"namespace":{"const":namespace}},"required":["namespace"]}
            },"required":["kind","subresource"]},
            "then":{"properties":{
                "data":{"allOf":[
                    {"$ref":format!("#/$defs/{PAYLOAD}")},
                    {"properties":{"record_type":{"const":record_type}},"required":["record_type"]}
                ]},
                "subresource":{"properties":{"indices":{"minItems":count,"maxItems":count}}}
            },"required":["data"]}
        }));
    }
    Ok(())
}
