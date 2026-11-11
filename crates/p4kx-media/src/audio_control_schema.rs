// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::audio_control_graph::rows as graph;
use crate::audio_control_payload::{self as wire, AtlPayload};
use crate::audio_control_reference::paths;
use crate::{MediaError, schema_merge};

const PAYLOAD: &str = "AtlPayload";

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    schema_merge::insert_payload(
        &mut base,
        PAYLOAD,
        schema_merge::serialized_payload::<AtlPayload<'static>>(),
    )?;
    let branches = base
        .get_mut("allOf")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MediaError::Contract("media schema has no built-in constraints".into()))?;
    for (namespace, record_type, indices, statuses) in [
        (wire::DOCUMENT, "document", 0, &["complete", "partial"][..]),
        (wire::ELEMENT, "element", 1, &["complete"][..]),
        (wire::DECLARATION, "declaration", 2, &["complete"][..]),
        (wire::CONTENT, "content", 1, &["complete"][..]),
        (wire::BINARY_TEXT, "binary-text", 1, &["complete"][..]),
        (wire::DEFINITION, "definition", 1, &["complete"][..]),
        (wire::FIELD, "field", 2, &["complete"][..]),
        (wire::REFERENCE, "reference", 2, &["complete"][..]),
        (
            graph::RESOLUTION,
            "reference-resolution",
            1,
            &["complete"][..],
        ),
        (
            graph::CANDIDATE,
            "reference-candidate",
            2,
            &["complete"][..],
        ),
        (
            paths::PATH_CANDIDATE,
            "path-candidate",
            3,
            &["complete"][..],
        ),
        (wire::FAILURE, "failure", 0, &["failed"][..]),
    ] {
        branches.push(constraint(namespace, record_type, indices, statuses));
    }
    Ok(base)
}

fn constraint(namespace: &str, record_type: &str, indices: u8, statuses: &[&str]) -> Value {
    json!({
        "if": {
            "properties": {
                "kind":{"const":"audio-control"},
                "subresource":{"properties":{"namespace":{"const":namespace}},"required":["namespace"]}
            },
            "required":["kind","subresource"]
        },
        "then": {"properties": {
            "data":{"allOf":[
                {"$ref":format!("#/$defs/{PAYLOAD}")},
                {"properties":{"record_type":{"const":record_type}},"required":["record_type"]}
            ]},
            "subresource":{"properties":{"indices":{"minItems":indices,"maxItems":indices}}},
            "status":{"enum":statuses}
        }, "required":["data", "status"]}
    })
}
