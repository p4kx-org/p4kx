// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::companions::{self, CompanionRecord};
use super::lod_distances;
use super::payload::{self, ModelSourcePayload};
use crate::{MediaError, schema_merge};

const PAYLOAD: &str = "ModelSourcePayload";
const COMPANION: &str = "ModelCompanionRecord";

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    schema_merge::insert_payload(
        &mut base,
        PAYLOAD,
        schema_merge::serialized_payload::<ModelSourcePayload>(),
    )?;
    schema_merge::insert_payload(
        &mut base,
        COMPANION,
        schema_merge::serialized_payload::<CompanionRecord>(),
    )?;
    let branches = base
        .get_mut("allOf")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MediaError::Contract("media schema has no built-in constraints".into()))?;
    for (namespace, record_type, count, definition) in [
        (payload::ENTRY, "entry", 0, PAYLOAD),
        (payload::CHUNK, "chunk", 1, PAYLOAD),
        (lod_distances::NAMESPACE, "lod-distance-words", 1, PAYLOAD),
        (companions::NAMESPACE, "companion", 1, COMPANION),
    ] {
        branches.push(json!({
            "if": {
                "properties": {
                    "kind":{"const":"model"},
                    "subresource":{"properties":{"namespace":{"const":namespace}},"required":["namespace"]}
                },
                "required":["kind","subresource"]
            },
            "then": {"properties":{
                "data":{"allOf":[
                    {"$ref":format!("#/$defs/{definition}")},
                    {"properties":{"record_type":{"const":record_type}},"required":["record_type"]}
                ]},
                "subresource":{"properties":{"indices":{"minItems":count,"maxItems":count}}}
            },"required":["data"]}
        }));
    }
    super::physics::bind(&mut base)?;
    Ok(base)
}
