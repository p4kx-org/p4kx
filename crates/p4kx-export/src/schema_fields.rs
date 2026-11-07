// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::Schema;

/// Requires every serialized object field, including fields whose value can be null.
pub(crate) fn every_field_required(schema: &mut Schema) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    let names: Vec<serde_json::Value> = object
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .map(|properties| {
            properties
                .keys()
                .cloned()
                .map(serde_json::Value::String)
                .collect()
        })
        .unwrap_or_default();
    object.insert("required".to_owned(), serde_json::Value::Array(names));
}
