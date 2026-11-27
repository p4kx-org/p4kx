// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::payload::{self as wire, AnimationPayload};
use crate::{MediaError, schema_merge};

const PAYLOAD: &str = "AnimationPayload";

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    schema_merge::insert_payload(
        &mut base,
        PAYLOAD,
        schema_merge::serialized_payload::<AnimationPayload<'static>>(),
    )?;
    let branches = base
        .get_mut("allOf")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MediaError::Contract("media schema has no built-in constraints".into()))?;
    for (namespace, record_type, indices) in [
        (wire::ENTRY, "entry", 0),
        (wire::CHUNK, "chunk", 1),
        (wire::CLIP, "clip", 1),
        (wire::DBA_METADATA, "dba-metadata", 1),
        (wire::IMAGE_INDEX, "image-index", 1),
        (wire::IMAGE_RECORD, "image-record", 2),
        (wire::IMAGE_DIRECTIONAL_PATH, "image-directional-path", 2),
        (
            wire::IMAGE_DIRECTIONAL_RECORD,
            "image-directional-record",
            2,
        ),
        (
            wire::IMAGE_DIRECTIONAL_POSE_INDEX,
            "image-directional-pose-index",
            2,
        ),
        (
            wire::IMAGE_DIRECTIONAL_POSE_BLOCK,
            "image-directional-pose-block",
            3,
        ),
        (wire::CONTROLLER, "controller", 2),
        (wire::SKELETON, "skeleton", 0),
        (wire::BINDING, "controller-binding", 2),
    ] {
        branches.push(json!({
            "if": {
                "properties": {
                    "kind":{"const":"animation"},
                    "subresource":{"properties":{"namespace":{"const":namespace}},"required":["namespace"]}
                },
                "required":["kind","subresource"]
            },
            "then": {"properties": {
                "data":{"allOf":[
                    {"$ref":format!("#/$defs/{PAYLOAD}")},
                    {"properties":{"record_type":{"const":record_type}},"required":["record_type"]}
                ]},
                "subresource":{"properties":{"indices":{"minItems":indices,"maxItems":indices}}}
            }, "required":["data"]}
        }));
    }
    super::entity_catalogue::bind_schema(&mut base)?;
    Ok(base)
}
