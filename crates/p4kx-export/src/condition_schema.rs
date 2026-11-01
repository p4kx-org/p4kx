// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Schema-only definition of the deprecated `other` location condition.

use schemars::Schema;
use serde_json::{Value, json};

/// Appends the deprecated `other` condition to the `oneOf` of `LocationCondition`.
///
/// No model type constructs it and the catalog never writes it: a condition of an unknown struct
/// fails the Record. The shape stays in the schema for readers of catalogs that predate that rule.
pub(crate) fn with_deprecated_other(schema: &mut Schema) {
    let Some(Value::Array(variants)) = schema
        .as_object_mut()
        .and_then(|object| object.get_mut("oneOf"))
    else {
        return;
    };
    variants.push(json!({
        "deprecated": true,
        "description": "Never written; a condition of an unknown struct fails the Record.",
        "properties": {
            "condition": {"const": "other", "type": "string"},
            "structName": {"description": "The struct name.", "type": "string"}
        },
        "required": ["condition", "structName"],
        "type": "object"
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_deprecated_variant_joins_the_one_of() {
        let mut schema = Schema::try_from(json!({"oneOf": [{"type": "object"}]})).expect("schema");
        with_deprecated_other(&mut schema);
        let value = schema.to_value();
        let variants = value["oneOf"].as_array().expect("oneOf");
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[1]["deprecated"], json!(true));
        assert_eq!(variants[1]["properties"]["condition"]["const"], "other");
    }

    #[test]
    fn a_schema_without_a_one_of_stays_untouched() {
        let mut schema = Schema::try_from(json!({"type": "string"})).expect("schema");
        with_deprecated_other(&mut schema);
        assert_eq!(schema.to_value(), json!({"type": "string"}));
    }
}
