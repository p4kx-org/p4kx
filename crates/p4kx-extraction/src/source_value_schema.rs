// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::Schema;

/// Couples the source state to its generic value without erasing nullable T.
pub(crate) fn correlate(schema: &mut Schema) {
    crate::field_origin::correlate_schema(schema);
    let Some(value) = schema
        .get_mut("properties")
        .and_then(|properties| properties.as_object_mut())
        .and_then(|properties| properties.insert("value".into(), true.into()))
    else {
        *schema = Schema::from(false);
        return;
    };
    let condition = schemars::json_schema!({
        "if": {"properties": {"state": {"const": "present"}}, "required": ["state"]},
        "then": {"properties": {"value": value}},
        "else": {"properties": {"value": {"type": "null"}}}
    });
    if let Some(all) = schema
        .get_mut("allOf")
        .and_then(|value| value.as_array_mut())
    {
        all.push(condition.into());
    } else {
        schema.insert("allOf".into(), vec![condition].into());
    }
}
