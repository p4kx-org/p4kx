// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Serialization-schema transform for extensible string vocabularies.

use schemars::Schema;
use serde_json::{Map, Value, json};

/// Describes a string vocabulary with documented values and accepts unknown strings.
pub(crate) fn open_string_vocabulary(schema: &mut Schema) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    let Some(values) = known_values(object) else {
        return;
    };
    object.remove("oneOf");
    object.remove("enum");
    object.insert("type".to_owned(), json!("string"));
    object.insert("x-values".to_owned(), Value::Array(values));
    object.insert("x-p4kx-vocabularyClass".to_owned(), json!("open"));
}

/// Extracts documented string constants from a unit-variant enum schema.
fn known_values(object: &Map<String, Value>) -> Option<Vec<Value>> {
    if let Some(Value::Array(variants)) = object.get("oneOf") {
        return variants.iter().map(variant_entry).collect();
    }
    let Some(Value::Array(values)) = object.get("enum") else {
        return None;
    };
    values
        .iter()
        .map(|value| value.is_string().then(|| json!({ "value": value })))
        .collect()
}

/// Extracts one documented string constant.
fn variant_entry(variant: &Value) -> Option<Value> {
    let value = variant.get("const").filter(|value| value.is_string())?;
    let mut entry = Map::new();
    entry.insert("value".to_owned(), value.clone());
    if let Some(description) = variant.get("description") {
        entry.insert("description".to_owned(), description.clone());
    }
    Some(Value::Object(entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transformed(schema: Value) -> Value {
        let mut schema = Schema::try_from(schema).expect("an object schema");
        open_string_vocabulary(&mut schema);
        schema.to_value()
    }

    #[test]
    fn documented_constants_form_an_open_vocabulary() {
        let schema = transformed(json!({
            "description": "Set.",
            "oneOf": [
                {"const": "a", "description": "The a.", "type": "string"},
                {"const": "b", "type": "string"}
            ]
        }));
        assert_eq!(
            schema,
            json!({
                "description": "Set.",
                "type": "string",
                "x-values": [{"value": "a", "description": "The a."}, {"value": "b"}],
                "x-p4kx-vocabularyClass": "open"
            })
        );
    }

    #[test]
    fn enum_values_form_an_open_vocabulary() {
        let schema = transformed(json!({"type": "string", "enum": ["a", "b"]}));
        assert_eq!(
            schema,
            json!({
                "type": "string", "x-values": [{"value": "a"}, {"value": "b"}],
                "x-p4kx-vocabularyClass": "open"
            })
        );
    }

    #[test]
    fn a_payload_union_stays_untouched() {
        let mixed = json!({
            "oneOf": [
                {"const": "a", "type": "string"},
                {"type": "object", "properties": {}}
            ]
        });
        assert_eq!(transformed(mixed.clone()), mixed);
    }

    #[test]
    fn non_string_constants_stay_untouched() {
        let numbers = json!({"oneOf": [{"const": 1}, {"const": 2}]});
        assert_eq!(transformed(numbers.clone()), numbers);
        let enum_numbers = json!({"enum": [1, 2]});
        assert_eq!(transformed(enum_numbers.clone()), enum_numbers);
    }
}
