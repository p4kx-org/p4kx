// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::FLAGS;
use crate::shader_material_support::{self as support, TestResult};

#[test]
fn material_shader_references_validate_in_the_actual_builtin_schema() -> TestResult {
    let result = support::run(
        vec![
            support::material("demo", "%A"),
            support::container("Data/shaders.pak", "demo", FLAGS),
        ],
        &support::options(),
    )?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &result.material_rows {
        assert!(validator.is_valid(&serde_json::to_value(row)?));
    }
    Ok(())
}

#[test]
fn material_shader_schema_requires_each_field_for_both_reference_roles() -> TestResult {
    let original = material_shader_schema_record()?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    assert!(
        validator.is_valid(&original),
        "the production record is valid"
    );
    for role in ["shader", "flag"] {
        let index = reference_index(&original, role)?;
        for field in [
            "material",
            "attribute_index",
            "token_index",
            "role",
            "shader",
            "reference",
            "input",
        ] {
            let mut missing = original.clone();
            reference_object_mut(&mut missing, index)?
                .remove(field)
                .ok_or("required fixture field")?;
            assert!(
                !validator.is_valid(&missing),
                "{role}.{field} cannot disappear from a generated material record"
            );
        }
    }
    Ok(())
}

#[test]
fn material_shader_schema_keeps_role_specific_nullable_values() -> TestResult {
    let original = material_shader_schema_record()?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for role in ["shader", "flag"] {
        let index = reference_index(&original, role)?;
        let link = original
            .pointer("/data/payload/shader_references")
            .and_then(Value::as_array)
            .and_then(|links| links.get(index))
            .ok_or("actual reference")?;
        assert!(
            link.get("input").is_some_and(Value::is_object),
            "actual lookup receipt"
        );
        let mut unread = original.clone();
        reference_object_mut(&mut unread, index)?.insert("input".into(), Value::Null);
        assert!(validator.is_valid(&unread), "{role} input stays nullable");
    }
    let shader = reference_index(&original, "shader")?;
    let mut untokened = original.clone();
    reference_object_mut(&mut untokened, shader)?.insert("token_index".into(), Value::Null);
    assert!(
        validator.is_valid(&untokened),
        "a shader reference has no token ordinal"
    );
    let flag = reference_index(&original, "flag")?;
    let mut unscoped = original;
    reference_object_mut(&mut unscoped, flag)?.insert("shader".into(), Value::Null);
    assert!(
        validator.is_valid(&unscoped),
        "a flag's authored scope stays nullable"
    );
    Ok(())
}

#[test]
fn material_shader_schema_rejects_wrong_role_and_nullable_branch_values() -> TestResult {
    let original = material_shader_schema_record()?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    assert!(
        validator.is_valid(&original),
        "the production record is valid"
    );
    for (role, field, value) in [
        ("shader", "token_index", json!(0)),
        ("shader", "shader", Value::Null),
        ("flag", "token_index", Value::Null),
        ("flag", "token_index", json!(-1)),
        ("flag", "token_index", json!(false)),
        ("flag", "shader", json!(false)),
        ("shader", "input", json!(false)),
        ("flag", "input", json!(false)),
        ("shader", "role", json!("unknown")),
        ("flag", "role", json!("Flag")),
    ] {
        let index = reference_index(&original, role)?;
        let mut changed = original.clone();
        reference_object_mut(&mut changed, index)?.insert(field.into(), value);
        assert!(
            !validator.is_valid(&changed),
            "the generated schema rejects the wrong {role}.{field} branch value"
        );
    }
    Ok(())
}

fn material_shader_schema_record() -> Result<Value, Box<dyn std::error::Error>> {
    let result = support::run(
        vec![
            support::material("demo", "%A"),
            support::container("Data/shaders.pak", "demo", FLAGS),
        ],
        &support::options(),
    )?;
    assert!(
        result.index.complete,
        "the schema fixture is a complete export"
    );
    let record = result
        .materials
        .first()
        .ok_or("actual material source record")?;
    Ok(serde_json::to_value(record)?)
}

fn reference_index(record: &Value, role: &str) -> Result<usize, Box<dyn std::error::Error>> {
    record
        .pointer("/data/payload/shader_references")
        .and_then(Value::as_array)
        .and_then(|links| {
            links
                .iter()
                .position(|link| link.get("role").and_then(Value::as_str) == Some(role))
        })
        .ok_or_else(|| "actual reference role".into())
}

fn reference_object_mut(
    record: &mut Value,
    index: usize,
) -> Result<&mut serde_json::Map<String, Value>, Box<dyn std::error::Error>> {
    record
        .pointer_mut("/data/payload/shader_references")
        .and_then(Value::as_array_mut)
        .and_then(|links| links.get_mut(index))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "actual reference object".into())
}
