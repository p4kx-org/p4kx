// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Required captured encryption in the actual shared use and inventory schemas.

use jsonschema::Validator;
use p4kx_extraction::{
    InputScope, SourceInput, SourceInventory, SourceUseDescription, SourceUseRole, SourceUses,
};
use serde_json::{Value, json};

use crate::source_use_encryption as evidence;
use crate::source_use_encryption::TestResult;

#[test]
fn owning_use_and_inventory_schemas_require_closed_effective_encryption() -> TestResult {
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let used = evidence::used(&scope, SourceUseRole::Payload)?;
    assert_eq!(
        used.reference().as_str(),
        "u2:u0:payload:payload:m4:fact",
        "captured evidence does not change the existing use ID algorithm"
    );
    check_schema(
        &schemars::schema_for!(SourceUseDescription).to_value(),
        serde_json::to_value(used.description())?,
        "",
    )?;
    check_schema(
        &schemars::schema_for!(SourceInventory).to_value(),
        serde_json::to_value(sources.freeze()?)?,
        "/uses/0",
    )
}

fn check_schema(schema: &Value, mut wire: Value, row_pointer: &str) -> TestResult {
    let validator = jsonschema::validator_for(schema)?;
    assert_eq!(
        wire.pointer(row_pointer)
            .ok_or("actual use row")?
            .get("effectiveEncryption"),
        Some(&json!("unknown")),
        "the actual current writer always includes captured use evidence"
    );
    assert!(
        validator.is_valid(&wire),
        "the actual typed use row validates"
    );
    let field_pointer = format!("{row_pointer}/effectiveEncryption");
    for state in ["encrypted", "unencrypted", "unknown"] {
        *wire
            .pointer_mut(&field_pointer)
            .ok_or("captured state field")? = json!(state);
        assert!(validator.is_valid(&wire), "closed existing state {state}");
    }
    wire.pointer_mut(row_pointer)
        .and_then(Value::as_object_mut)
        .ok_or("use object")?
        .remove("effectiveEncryption");
    assert!(
        !validator.is_valid(&wire),
        "current use evidence is required"
    );
    wire.pointer_mut(row_pointer)
        .and_then(Value::as_object_mut)
        .ok_or("use object")?
        .insert("effectiveEncryption".into(), json!("unknown"));
    check_invalid_states(&validator, &mut wire, row_pointer, &field_pointer)
}

fn check_invalid_states(
    validator: &Validator,
    wire: &mut Value,
    row_pointer: &str,
    field_pointer: &str,
) -> TestResult {
    for invalid in [
        json!(null),
        json!(true),
        json!(0),
        json!(""),
        json!("future"),
    ] {
        *wire
            .pointer_mut(field_pointer)
            .ok_or("captured state field")? = invalid;
        assert!(
            !validator.is_valid(wire),
            "use evidence keeps its closed control type"
        );
    }
    *wire
        .pointer_mut(field_pointer)
        .ok_or("captured state field")? = json!("unknown");
    wire.pointer_mut(row_pointer)
        .and_then(Value::as_object_mut)
        .ok_or("use object")?
        .insert("futureMetadata".into(), json!({"additional": true}));
    assert!(
        validator.is_valid(wire),
        "unknown additive metadata remains allowed"
    );
    Ok(())
}
