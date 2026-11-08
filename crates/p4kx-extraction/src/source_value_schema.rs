// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::Schema;

/// Couples the source state to its generic value without erasing nullable T.
pub(crate) fn correlate(schema: &mut Schema) {
    crate::projection_schema::require_properties(schema);
    let Some(value) = schema
        .get("properties")
        .and_then(|properties| properties.get("value"))
        .cloned()
    else {
        *schema = Schema::from(false);
        return;
    };
    if let Some(properties) = schema
        .get_mut("properties")
        .and_then(|properties| properties.as_object_mut())
    {
        properties.insert("value".into(), true.into());
    }
    let condition = schemars::json_schema!({
        "if": {"properties": {"state": {"const": "present"}}, "required": ["state"]},
        "then": {"properties": {"value": value}},
        "else": {"properties": {"value": {"type": "null"}}}
    });
    let mut all = schema
        .get("allOf")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    all.push(condition.into());
    schema.insert("allOf".into(), all.into());
}
