// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Map, Value, json};

use super::document::{MARKER_FORMAT, MARKER_FORMAT_VERSION};

const PROOF_FIELDS: &[&str] = &["sourceUsesRefs", "sourceSetComplete"];
const VARIANT_PROOF_FIELDS: &[&str] = &[
    "sourceUsesRefs",
    "sourceSetComplete",
    "sourceUseSetRef",
    "failedSourceUseRefs",
];

/// Returns the JSON Schema of the versioned marker-only catalog document.
#[must_use]
pub fn catalog_marker_schema() -> Value {
    let Value::Object(mut catalog) = super::catalog_schema() else {
        return Value::Bool(false);
    };
    let Some(Value::Object(mut definitions)) = catalog.remove("$defs") else {
        return Value::Bool(false);
    };
    let Some(draft) = catalog.remove("$schema") else {
        return Value::Bool(false);
    };
    remove_properties(&mut catalog, &["provenance"]);
    forbid_properties(&mut catalog, &["provenance"]);
    definitions.remove("CatalogProvenance");
    definitions.remove("VariantSourceUseSet");
    definitions.remove("VariantSourceUseSets");
    for (name, schema) in &mut definitions {
        if let Value::Object(schema) = schema {
            project_definition(name, schema);
        }
    }
    json!({
        "$schema": draft,
        "$defs": definitions,
        "title": "P4KX marker catalog",
        "type": "object",
        "required": ["format", "formatVersion", "catalog"],
        "additionalProperties": false,
        "properties": {
            "format": {"const": MARKER_FORMAT},
            "formatVersion": {"const": MARKER_FORMAT_VERSION},
            "catalog": catalog,
        },
    })
}

fn project_definition(name: &str, schema: &mut Map<String, Value>) {
    match name {
        "VariantDecision" => {
            remove_properties(schema, VARIANT_PROOF_FIELDS);
            forbid_properties(schema, VARIANT_PROOF_FIELDS);
            schema.insert(
                "allOf".into(),
                json!([{
                    "if": {"properties": {"state": {"const": "present"}}, "required": ["state"]},
                    "then": {"properties": {"value": {"type": "object"}}},
                    "else": {"properties": {"value": {"type": "null"}}}
                }]),
            );
        }
        "ExportDiagnosticExample" => {
            remove_properties(schema, &["failedSourceUses"]);
            forbid_properties(schema, &["failedSourceUses"]);
        }
        "FieldOrigin" | "ObjectiveBinding" | "PropertyBinding" => project_origin(schema),
        "DeclarationBinding" | "FlowFieldBinding" => project_origin_alternatives(schema),
        name if name.starts_with("SourcedValueOf") => project_origin(schema),
        _ => {}
    }
}

fn project_origin_alternatives(schema: &mut Map<String, Value>) {
    forbid_properties(schema, PROOF_FIELDS);
    let Some(alternatives) = schema.get_mut("oneOf").and_then(Value::as_array_mut) else {
        return;
    };
    for alternative in alternatives.iter_mut().filter_map(Value::as_object_mut) {
        if alternative
            .get("properties")
            .and_then(|properties| properties.get("containsEncryptedSource"))
            .is_some()
        {
            remove_origin_proofs(alternative);
        }
    }
}

fn project_origin(schema: &mut Map<String, Value>) {
    remove_origin_proofs(schema);
    forbid_properties(schema, PROOF_FIELDS);
}

fn remove_origin_proofs(schema: &mut Map<String, Value>) {
    remove_properties(schema, PROOF_FIELDS);
    if let Some(combined) = schema.get_mut("allOf").and_then(Value::as_array_mut) {
        combined.retain(|condition| condition.pointer("/properties/sourceUsesRefs").is_none());
        if combined.is_empty() {
            schema.remove("allOf");
        }
    }
}

fn remove_properties(schema: &mut Map<String, Value>, fields: &[&str]) {
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        for field in fields {
            properties.remove(*field);
        }
    }
    if let Some(required) = schema.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|field| field.as_str().is_none_or(|name| !fields.contains(&name)));
    }
}

fn forbid_properties(schema: &mut Map<String, Value>, fields: &[&str]) {
    let alternatives: Vec<_> = fields
        .iter()
        .map(|field| json!({"required": [field]}))
        .collect();
    schema.insert("not".into(), json!({"anyOf": alternatives}));
}
