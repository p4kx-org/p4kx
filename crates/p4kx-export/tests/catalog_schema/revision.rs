// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::{Value, json};

use crate::catalog_schema_support::minimal_catalog;

type TestResult = Result<(), Box<dyn Error>>;

fn mission_item(catalog: &mut Value) -> Result<&mut Value, Box<dyn Error>> {
    catalog
        .get_mut("contractTemplates")
        .and_then(|templates| templates.get_mut(0))
        .and_then(|template| template.get_mut("contractProperties"))
        .and_then(Value::as_array_mut)
        .and_then(|properties| {
            properties.iter_mut().find(|property| {
                property
                    .get("value")
                    .and_then(|value| value.get("type"))
                    .and_then(Value::as_str)
                    == Some("missionItem")
            })
        })
        .and_then(|property| property.get_mut("value"))
        .ok_or_else(|| "the fixture has a mission item property".into())
}

#[test]
fn the_schema_version_has_one_authoritative_value() {
    let schema = p4kx_export::catalog_schema();
    assert_eq!(p4kx_export::SCHEMA_VERSION, 3, "the approved v3 contract");
    assert_eq!(
        schema["properties"]["schemaVersion"]["const"], 3,
        "schema and writer share the version"
    );
}

#[test]
fn nullable_tagged_payload_fields_are_present_even_when_null() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let mut catalog = minimal_catalog();
    mission_item(&mut catalog)?["forcedItemGuid"] = Value::Null;
    assert!(validator.is_valid(&catalog), "an explicit null is valid");
    mission_item(&mut catalog)?
        .as_object_mut()
        .ok_or("a property value is an object")?
        .remove("forcedItemGuid");
    assert!(
        !validator.is_valid(&catalog),
        "an omitted payload is invalid"
    );
    Ok(())
}

#[test]
fn a_tagged_union_extension_is_not_an_additive_property() -> TestResult {
    let released: Value =
        serde_json::from_str(include_str!("../fixtures/catalog-v0.4.2.schema.json"))?;
    let validator = jsonschema::validator_for(&released)?;
    let mut catalog = minimal_catalog();
    catalog["schemaVersion"] = json!(2);
    catalog["contractTemplates"][0]["objectives"][0]["objectiveProperties"] = json!([]);
    assert!(validator.is_valid(&catalog), "unchanged v2 fields validate");

    catalog["resourceTypes"] = json!([]);
    assert!(validator.is_valid(&catalog), "a new kind alone is additive");

    mission_item(&mut catalog)?["type"] = json!("scopedLocation");
    assert!(
        !validator.is_valid(&catalog),
        "an old closed union rejects a new variant even with unknown fields allowed"
    );
    Ok(())
}

#[test]
fn v3_and_v2_are_not_mislabeled_as_each_other() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let mut catalog = minimal_catalog();
    assert!(
        validator.is_valid(&catalog),
        "a complete v3 document validates"
    );
    catalog["schemaVersion"] = json!(2);
    assert!(
        !validator.is_valid(&catalog),
        "a v2 document requires its own schema"
    );
    Ok(())
}
