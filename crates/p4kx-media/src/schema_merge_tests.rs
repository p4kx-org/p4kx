// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::{MediaError, archive_metadata_schema, schema_merge, shader_schema};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn payload_definitions_share_equal_names_and_keep_root_relative_references() -> TestResult {
    let mut base = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": {"Shared": {"type": "string"}}
    });
    let payload = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": {"Shared": {"type": "string"}, "Allowed": true},
        "type": "object",
        "properties": {"field": {"$ref": "#/$defs/Shared"}}
    });
    schema_merge::insert_payload(&mut base, "Payload", payload)?;
    assert_eq!(
        base.pointer("/$defs/Shared"),
        Some(&json!({"type": "string"}))
    );
    assert_eq!(base.pointer("/$defs/Allowed"), Some(&json!(true)));
    assert_eq!(
        base.pointer("/$defs/Payload/properties/field/$ref"),
        Some(&json!("#/$defs/Shared"))
    );
    assert!(base.pointer("/$defs/Payload/$defs").is_none());
    assert!(base.pointer("/$defs/Payload/$schema").is_none());
    assert!(base.get("$schema").is_some());
    Ok(())
}

#[test]
fn conflicting_payload_and_nested_definition_names_are_refused() {
    for (mut base, payload) in [
        (
            json!({"$defs": {"Payload": {"type": "integer"}}}),
            json!({"type": "string"}),
        ),
        (
            json!({"$defs": {"Shared": {"type": "integer"}}}),
            json!({"$defs": {"Shared": {"type": "string"}}}),
        ),
    ] {
        assert!(
            matches!(
                schema_merge::insert_payload(&mut base, "Payload", payload),
                Err(MediaError::Contract(_))
            ),
            "a conflicting definition cannot produce a usable bound schema"
        );
    }
}

#[test]
fn malformed_roots_definitions_and_definition_values_are_refused() {
    for (mut base, payload) in [
        (Value::Null, json!({})),
        (json!([]), json!({})),
        (json!({"$defs": []}), json!({})),
        (json!({"$defs": null}), json!({})),
        (json!({}), Value::Null),
        (json!({}), json!(true)),
        (json!({}), json!([])),
        (json!({}), json!({"$defs": []})),
        (json!({}), json!({"$defs": null})),
        (json!({}), json!({"$defs": {"Invalid": 17}})),
        (json!({}), json!({"$defs": {"Invalid": null}})),
    ] {
        assert!(
            matches!(
                schema_merge::insert_payload(&mut base, "Payload", payload),
                Err(MediaError::Contract(_))
            ),
            "malformed merge input must return a typed contract error"
        );
    }
}

#[test]
fn both_binders_refuse_malformed_common_constraint_containers() {
    for base in [Value::Null, json!({"allOf": {}}), json!({"$defs": []})] {
        assert!(
            matches!(
                archive_metadata_schema::bind_record_schema(base.clone()),
                Err(MediaError::Contract(_))
            ),
            "Archive binding must fail closed"
        );
        assert!(
            matches!(
                shader_schema::bind_record_schema(base),
                Err(MediaError::Contract(_))
            ),
            "shader binding must fail closed"
        );
    }
}

#[test]
fn archive_payload_cannot_replace_an_unrelated_common_definition() {
    assert!(
        matches!(
            archive_metadata_schema::bind_record_schema(json!({
                "$defs": {"EntryMetadataInfo": {"type": "array"}}
            })),
            Err(MediaError::Contract(_))
        ),
        "generated metadata cannot silently replace a common schema definition"
    );
}
