// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::EntryMetadataInfo;
use serde_json::Value;

use crate::{MediaError, archive_provider, schema_merge};

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    schema_merge::insert_payload(
        &mut base,
        "EntryMetadataInfo",
        schemars::schema_for!(EntryMetadataInfo).to_value(),
    )?;
    let root = base
        .as_object_mut()
        .ok_or_else(|| MediaError::Contract("media record schema is not an object".into()))?;
    let constraints = root
        .entry("allOf")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| MediaError::Contract("media record allOf is not an array".into()))?;
    constraints.push(serde_json::json!({
        "if": {
            "properties": {
                "kind": {"const": "archive"},
                "subresource": {
                    "properties": {"namespace": {"const": archive_provider::ENTRY_NAMESPACE}},
                    "required": ["namespace"]
                }
            },
            "required": ["kind", "subresource"]
        },
        "then": {
            "properties": {
                "data": {
                    "type": "object",
                    "properties": {"metadata": {"$ref": "#/$defs/EntryMetadataInfo"}},
                    "required": ["metadata"]
                }
            },
            "required": ["data"]
        }
    }));
    Ok(base)
}
