// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::character_payload;
use super::companions::{self, CompanionRecord};
use super::lod_distances;
use super::mesh_record;
use super::payload::{self, ModelSourcePayload};
use super::skeleton_record;
use super::vehicle_grid_record;
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
    character_payload::insert_schema(&mut base)?;
    skeleton_record::insert_schema(&mut base)?;
    mesh_record::insert_schema(&mut base)?;
    vehicle_grid_record::insert_schema(&mut base)?;
    let branches = base
        .get_mut("allOf")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MediaError::Contract("media schema has no built-in constraints".into()))?;
    for (namespace, record_type, count, definition) in [
        (payload::ENTRY, "entry", 0, PAYLOAD),
        (payload::CHUNK, "chunk", 1, PAYLOAD),
        (lod_distances::NAMESPACE, "lod-distance-words", 1, PAYLOAD),
        (companions::NAMESPACE, "companion", 1, COMPANION),
    ]
    .into_iter()
    .chain(character_payload::SCHEMA_ROWS)
    .chain(skeleton_record::SCHEMA_ROWS)
    .chain(mesh_record::SCHEMA_ROWS)
    .chain(vehicle_grid_record::SCHEMA_ROWS)
    {
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
