// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::payload::{self as wire, DefinitionPayload};
use crate::{MediaError, schema_merge};

const PAYLOAD: &str = "AnimationDefinitionPayload";

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    schema_merge::insert_payload(
        &mut base,
        PAYLOAD,
        schema_merge::serialized_payload::<DefinitionPayload<'static>>(),
    )?;
    let branches = base
        .get_mut("allOf")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MediaError::Contract("media schema has no built-in constraints".into()))?;
    for (namespace, record_type, indices) in [
        (wire::DOCUMENT, "document", 0),
        (wire::ELEMENT, "element", 1),
        (wire::DECLARATION, "declaration", 2),
        (wire::CONTENT, "content", 1),
        (wire::BINARY_TEXT, "binary-text", 1),
        (wire::LINK, "link", 2),
        (wire::NAMED_CAF, "named-caf", 2),
        (wire::NAMED_DBA_BINDING, "named-dba-binding", 2),
        (wire::EVENT_TIMELINE, "event-timeline", 1),
        (wire::EVENT, "event", 1),
        (wire::EVENT_ATTRIBUTE, "event-attribute", 2),
        (wire::GRAPH_NODE, "graph-node", 1),
        (wire::GRAPH_ATTRIBUTE, "graph-attribute", 2),
    ] {
        branches.push(json!({
            "if": {"properties": {
                "kind": {"const": "animation-definition"},
                "subresource": {"properties": {"namespace": {"const": namespace}}, "required": ["namespace"]}
            }, "required": ["kind", "subresource"]},
            "then": {"properties": {
                "data": {"allOf": [
                    {"$ref": format!("#/$defs/{PAYLOAD}")},
                    {"properties": {"record_type": {"const": record_type}}, "required": ["record_type"]}
                ]},
                "subresource": {"properties": {"indices": {"minItems": indices, "maxItems": indices}}}
            }, "required": ["data"]}
        }));
    }
    Ok(base)
}
