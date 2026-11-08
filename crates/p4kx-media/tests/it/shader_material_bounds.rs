// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::ConflictPolicy;
use p4kx_media::MediaError;
use serde_json::{Value, json};

use super::shader_material_support::{self as support, TestResult};

const FLAGS: &[u8] = b"Property { Name = %A\n }\n";

#[test]
fn candidate_limit_remains_unavailable_and_never_selects_one_duplicate() -> TestResult {
    let mut options = support::options();
    options.limits.reference_candidates = 1;
    let result = support::run(
        vec![
            support::material("demo", "%A"),
            support::container("Data/a.pak", "demo", FLAGS),
            support::container("Data/b.pak", "demo", FLAGS),
        ],
        &options,
    )?;
    let links = support::links(&result)?;
    for link in links {
        assert_eq!(link["reference"]["resolution"]["status"], "unavailable");
        assert_eq!(
            link["reference"]["resolution"]["reason"]["type"],
            "candidate-limit"
        );
        assert_eq!(link["reference"]["candidates_complete"], false);
    }
    Ok(())
}

#[test]
fn failed_definition_parse_retains_its_actual_lookup_receipt_and_partial_generation() -> TestResult
{
    let mut options = support::options();
    options.diagnostic = true;
    let result = support::run(
        vec![
            support::material("demo", "%A"),
            support::container(
                "Data/shaders.pak",
                "demo",
                b"Property { Name = \"unterminated",
            ),
        ],
        &options,
    )?;
    assert!(!result.index.complete);
    for link in support::links(&result)? {
        assert_eq!(link["reference"]["resolution"]["status"], "unavailable");
        assert_eq!(
            link["reference"]["resolution"]["reason"]["type"],
            "unsupported-layout"
        );
        assert_eq!(link["input"]["omission"], "reference-lookup");
    }
    Ok(())
}

#[test]
fn generation_work_exhaustion_during_lookup_preserves_the_last_complete_index() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = support::archive(
        directory.path(),
        vec![
            support::material("demo", "%A"),
            support::container("Data/shaders.pak", "demo", FLAGS),
        ],
    )?;
    let output = directory.path().join("media");
    let options = support::options();
    p4kx_media::export(&archive, &output, &options)?;
    let before = std::fs::read(output.join("media-index.json"))?;
    let mut limited = options;
    limited.conflict = ConflictPolicy::Overwrite;
    limited.limits.metadata_work = 1;
    let failure = p4kx_media::export(&archive, &output, &limited)
        .err()
        .ok_or("lookup budget unexpectedly succeeded")?;
    assert!(
        metadata_limit(&failure),
        "the shared work limit remains typed: {failure}"
    );
    let MediaError::Source { entry, .. } = &failure else {
        return Err("lookup failure lost the requesting material".into());
    };
    assert!(
        matches!(&entry.identity, p4kx_media::Identity::Entry { entry, .. }
        if entry.path == "Data/materials.mtl"),
        "the exhausted lookup belongs to the requesting material"
    );
    assert_eq!(std::fs::read(output.join("media-index.json"))?, before);
    Ok(())
}

fn metadata_limit(error: &MediaError) -> bool {
    match error {
        MediaError::Limit {
            budget: "metadata_work",
            ..
        } => true,
        MediaError::Source { cause, .. } => metadata_limit(cause),
        _ => false,
    }
}

#[test]
fn repeated_exports_do_not_reuse_another_roots_definition_index() -> TestResult {
    for name in ["first", "second"] {
        let definition = format!("Property {{ Name = %{name}\n }}\n");
        let flags = format!("%{name}");
        let result = support::run(
            vec![
                support::material("demo", &flags),
                support::container("Data/shaders.pak", "demo", definition.as_bytes()),
            ],
            &support::options(),
        )?;
        assert_eq!(
            support::links(&result)?[1]["reference"]["resolution"]["status"],
            "resolved"
        );
    }
    Ok(())
}

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
    for row in &result.materials {
        assert!(validator.is_valid(&serde_json::to_value(row)?));
    }
    Ok(())
}
#[test]
fn long_retained_identity_paths_consume_the_shared_lookup_work_budget() -> TestResult {
    let long = "x".repeat(2048);
    let mut options = support::options();
    options.limits.metadata_work = 4096;
    let directory = tempfile::tempdir()?;
    let nested = p4kx_testkit::ArchiveBuilder::new()
        .entry(p4kx_testkit::TestEntry::stored(
            format!("Data/{long}/demo.ext"),
            FLAGS,
        ))
        .build();
    let archive = support::archive(
        directory.path(),
        vec![
            support::material("demo", "%A"),
            p4kx_testkit::TestEntry::stored(format!("Data/{long}.pak"), nested),
        ],
    )?;
    let failure = p4kx_media::export(&archive, &directory.path().join("media"), &options)
        .err()
        .ok_or("identity storage escaped lookup work budget")?;
    assert!(metadata_limit(&failure));
    Ok(())
}

#[test]
fn shader_crc_failure_preserves_both_requesting_material_and_actual_target_source() -> TestResult {
    let directory = tempfile::tempdir()?;
    let nested = p4kx_testkit::ArchiveBuilder::new()
        .entry(p4kx_testkit::TestEntry::stored("Data/demo.ext", FLAGS).with_crc32c(0))
        .build();
    let archive = support::archive(
        directory.path(),
        vec![
            support::material("demo", "%A"),
            p4kx_testkit::TestEntry::stored("Data/shaders.pak", nested),
        ],
    )?;
    let failure = p4kx_media::export(
        &archive,
        &directory.path().join("media"),
        &support::options(),
    )
    .err()
    .ok_or("invalid shader CRC was accepted")?;
    let MediaError::Source { entry, cause } = &failure else {
        return Err("requesting source was lost".into());
    };
    let p4kx_media::Identity::Entry { entry, .. } = &entry.identity else {
        return Err("material identity was lost".into());
    };
    assert!(entry.path.ends_with("materials.mtl"));
    assert!(
        has_shader_source(cause),
        "actual shader input source was lost"
    );
    Ok(())
}

fn has_shader_source(error: &MediaError) -> bool {
    match error {
        MediaError::Source { entry, cause } => {
            matches!(&entry.identity, p4kx_media::Identity::Entry { containers, entry }
                if !containers.is_empty() && entry.path.ends_with("demo.ext"))
                || has_shader_source(cause)
        }
        _ => false,
    }
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
