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
        .map_or_default(|properties| {
            properties
                .keys()
                .cloned()
                .map(serde_json::Value::String)
                .collect()
        });
    object.insert("required".to_owned(), serde_json::Value::Array(names));
}

/// Keeps string-only variants with documentation in the compact enum schema.
pub(crate) fn compact_string_enum(schema: &mut Schema) {
    use std::collections::BTreeSet;

    use serde_json::Value;

    let Some(object) = schema.as_object_mut() else {
        return;
    };
    let Some(variants) = object.get("oneOf").and_then(Value::as_array) else {
        return;
    };
    let mut seen = BTreeSet::new();
    let Some(values): Option<Vec<_>> = variants
        .iter()
        .map(|variant| {
            let fields = variant.as_object()?;
            if fields
                .keys()
                .any(|key| !matches!(key.as_str(), "type" | "const" | "description"))
                || fields.get("type")?.as_str()? != "string"
            {
                return None;
            }
            let value = fields.get("const")?.as_str()?;
            seen.insert(value).then(|| Value::String(value.to_owned()))
        })
        .collect()
    else {
        return;
    };
    if values.is_empty() {
        return;
    }
    object.remove("oneOf");
    object.insert("type".to_owned(), Value::String("string".to_owned()));
    object.insert("enum".to_owned(), Value::Array(values));
}
