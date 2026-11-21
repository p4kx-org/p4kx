// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::MaterialProvider;
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::material_support::{self as support, TestResult};

fn fixture() -> Result<Value, Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [TestEntry::stored(
            "source.mtl",
            b"<Material DefaultPalette='record/palette'><Unknown Keep='source'/></Material>",
        )],
    )?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &support::registry(MaterialProvider::default())?,
    )?)?;
    let record = support::records(&output, &index)?
        .into_iter()
        .next()
        .ok_or("material source record")?;
    Ok(serde_json::to_value(record)?)
}

#[test]
fn actual_material_source_payload_and_owned_namespace_validate() -> TestResult {
    let record = fixture()?;
    let record_validator = jsonschema::validator_for(&p4kx_media::material_record_schema()?)?;
    let payload_validator = jsonschema::validator_for(&p4kx_media::material_payload_schema())?;
    record_validator
        .validate(&record)
        .map_err(|error| error.to_string())?;
    payload_validator
        .validate(record.get("data").ok_or("payload")?)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn missing_source_fields_wrong_payloads_and_false_complete_status_are_rejected() -> TestResult {
    let original = fixture()?;
    let validator = jsonschema::validator_for(&p4kx_media::material_record_schema()?)?;
    for pointer in [
        "/data/payload/authored/root/syntax/qualified_name",
        "/data/payload/authored/root/syntax/attributes",
        "/data/payload/materials/0/authored/public_params",
    ] {
        let mut changed = original.clone();
        *changed
            .pointer_mut(pointer)
            .ok_or("required fixture field")? = Value::Null;
        assert!(
            !validator.is_valid(&changed),
            "typed source fields cannot silently disappear"
        );
    }
    let mut wrong = original.clone();
    *wrong
        .pointer_mut("/data/record_type")
        .ok_or("record_type")? = serde_json::json!("failure");
    assert!(
        !validator.is_valid(&wrong),
        "library namespace rejects a foreign payload variant"
    );
    let mut partial = original;
    *partial
        .pointer_mut("/data/payload/projection_complete")
        .ok_or("coverage")? = Value::Bool(false);
    assert!(
        !validator.is_valid(&partial),
        "partial structured projection cannot retain a complete status"
    );
    Ok(())
}

#[test]
fn missing_authored_root_cannot_hide_complete_material_source_loss() -> TestResult {
    let mut record = fixture()?;
    assert_eq!(
        record.get("status"),
        Some(&json!("complete")),
        "complete fixture"
    );
    record
        .pointer_mut("/data/payload/authored")
        .and_then(Value::as_object_mut)
        .ok_or("authored object")?
        .remove("root")
        .ok_or("authored root")?;
    let validator = jsonschema::validator_for(&p4kx_media::material_record_schema()?)?;
    assert!(
        !validator.is_valid(&record),
        "removing the nullable root cannot bypass the owned material completeness rules"
    );
    Ok(())
}

#[test]
fn authored_nullable_properties_are_present_and_still_accept_null() -> TestResult {
    let schema = p4kx_media::material_payload_schema();
    require_nullable_fields(
        &schema,
        "MaterialLibrarySourceInfo",
        &json!({"encoding":"text","root":null,"losses":[],"entries":[]}),
        &["root"],
    )?;
    require_nullable_fields(
        &schema,
        "MaterialElementSyntaxInfo",
        &json!({"ordinal":null,"qualified_name":"Material","namespace":null,"attributes":[],"content":[],"binary_text":null}),
        &["ordinal", "namespace", "binary_text"],
    )?;
    require_nullable_fields(
        &schema,
        "MaterialAttributeInfo",
        &json!({"name":"Keep","value":"raw","lexeme":null,"kind":{"type":"attribute","namespace":null}}),
        &["lexeme"],
    )?;
    Ok(())
}

#[test]
fn nullable_variant_properties_are_present_without_changing_their_null_type() -> TestResult {
    let schema = p4kx_media::material_payload_schema();
    for (value, property) in [
        (json!({"type":"attribute","namespace":null}), "namespace"),
        (json!({"type":"namespace","prefix":null}), "prefix"),
    ] {
        require_nullable_fields(&schema, "MaterialAttributeKindInfo", &value, &[property])?;
    }
    require_nullable_fields(
        &schema,
        "MaterialContentInfo",
        &json!({"type":"processing-instruction","target":"qa","value":null}),
        &["value"],
    )?;
    Ok(())
}

#[test]
fn new_media_nullable_properties_keep_automatic_nonnullable_requirements() -> TestResult {
    let record = fixture()?;
    let schema = p4kx_media::material_payload_schema();
    let payload = record.pointer("/data/payload").ok_or("library payload")?;
    for (name, value, properties) in [
        ("MaterialLibraryMetadata", payload, &["default_palette"][..]),
        (
            "MaterialReference",
            payload.pointer("/references/0").ok_or("root reference")?,
            &["material", "slot", "submaterial"][..],
        ),
        (
            "MaterialSurface",
            payload
                .pointer("/materials/0/interpreted_surface")
                .ok_or("surface")?,
            &["palette", "decal_colours", "diffuse_map_is_grey"][..],
        ),
    ] {
        require_nullable_fields(&schema, name, value, properties)?;
    }
    Ok(())
}

fn require_nullable_fields(
    schema: &Value,
    name: &str,
    value: &Value,
    properties: &[&str],
) -> TestResult {
    let isolated = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": format!("#/$defs/{name}"),
        "$defs": schema.get("$defs").ok_or("material schema definitions")?,
    });
    let validator = jsonschema::validator_for(&isolated)?;
    validator
        .validate(value)
        .map_err(|error| error.to_string())?;
    for property in properties {
        let mut missing = value.clone();
        missing
            .as_object_mut()
            .ok_or("object fixture")?
            .remove(*property)
            .ok_or("nullable field")?;
        assert!(
            !validator.is_valid(&missing),
            "{name}.{property} is always serialized and cannot disappear"
        );
        let mut null = value.clone();
        *null.get_mut(*property).ok_or("nullable field")? = Value::Null;
        assert!(
            validator.is_valid(&null),
            "{name}.{property} remains nullable when its key is present"
        );
    }
    Ok(())
}
