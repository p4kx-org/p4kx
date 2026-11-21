// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_schema;
use serde_json::Value;

use crate::commodity_v3_fixture::{self as fixture, TestResult};

#[test]
fn actual_canonical_output_validates_and_nullable_presentation_keys_are_required() -> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    let schema = catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(&catalog),
        "the actual public producer validates against its generated schema"
    );
    for field in [
        "name",
        "description",
        "displayType",
        "typeGuid",
        "subtypeGuid",
        "densityGramsPerCubicCentimetre",
        "refinedSubtypeGuid",
    ] {
        let mut invalid = catalog.clone();
        let rows = invalid
            .pointer_mut("/projections/commodityPresentation/rows")
            .and_then(Value::as_array_mut)
            .ok_or("presentation rows")?;
        let row = rows
            .iter_mut()
            .find(|row| row["commodityGuid"]["value"] == fixture::guid(fixture::UNLINKED))
            .ok_or("unlinked row")?;
        let carrier = row
            .get_mut(field)
            .and_then(Value::as_object_mut)
            .ok_or("sourced nullable field")?;
        assert_eq!(
            carrier.get("state").and_then(Value::as_str),
            Some("present")
        );
        assert!(
            carrier.get("value").is_some_and(Value::is_null),
            "no selected value is an explicit null"
        );
        assert!(carrier.remove("value").is_some());
        assert!(
            !validator.is_valid(&invalid),
            "nullable values still require the value key"
        );
    }
    Ok(())
}

#[test]
fn each_immediate_origin_key_and_closed_selection_control_is_enforced_by_the_schema() -> TestResult
{
    let (_, catalog) = fixture::build(&fixture::database())?;
    let schema = catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    for key in [
        "containsEncryptedSource",
        "sourceUsesRefs",
        "sourceSetComplete",
    ] {
        let mut invalid = catalog.clone();
        let carrier = invalid
            .pointer_mut("/definitions/commodities/rows/0/guid")
            .and_then(Value::as_object_mut)
            .ok_or("identity carrier")?;
        assert!(carrier.remove(key).is_some());
        assert!(
            !validator.is_valid(&invalid),
            "an immediate origin cannot be omitted"
        );
    }
    let mut invalid = catalog;
    let selection = invalid
        .pointer_mut("/definitions/commodities/rows/0/selection/value")
        .ok_or("selection value")?;
    *selection = Value::String("futureSuccessfulSelection".to_owned());
    assert!(
        !validator.is_valid(&invalid),
        "unknown selection controls cannot imply a successful state"
    );
    Ok(())
}

#[test]
fn protected_owner_fields_and_reference_closure_keep_shared_keys_in_separate_text_domains()
-> TestResult {
    let (_, catalog) = fixture::build(&fixture::with_keys("@shared", "@shared", "@resource_name"))?;
    let row = fixture::presentation(&catalog, fixture::ALPHA)?;
    let public = fixture::text_row(&catalog, "publicNames", &row["name"]["value"])?;
    let protected = fixture::text_row(&catalog, "gameTexts", &row["description"]["value"])?;
    assert_eq!(public["key"]["value"], "@shared");
    assert_eq!(protected["key"]["value"], "@shared");
    assert_ne!(
        row["name"]["value"], row["description"]["value"],
        "protection domains do not merge equal keys or bytes"
    );
    assert!(
        !serde_json::to_string(&catalog["provenance"])?.contains("@shared"),
        "global source metadata does not contain original text-key values"
    );
    Ok(())
}

#[test]
fn generated_schema_marks_each_protected_owner_and_the_referenced_text_domain() -> TestResult {
    let schema = catalog_schema();
    for (owner, field) in [
        ("CommodityPresentation", "description"),
        ("CommodityPresentation", "displayType"),
        ("CommodityDisplay", "displayDescription"),
        ("CommodityPurchasable", "displayType"),
    ] {
        let property = property(&schema, owner, field)?;
        assert_eq!(
            property["x-gameText"], true,
            "the protected owner reference is explicitly marked"
        );
        let value = present_value(&schema, property)?;
        assert!(
            marked(&schema, value, &mut BTreeSet::new())?,
            "the referenced value remains in the protected domain"
        );
    }
    let public_property = property(&schema, "CommodityPresentation", "name")?;
    let public_value = present_value(&schema, public_property)?;
    assert!(
        !marked(&schema, public_value, &mut BTreeSet::new())?,
        "the public short-name domain stays public"
    );
    assert_eq!(
        property(&schema, "CatalogTexts", "gameTexts")?["x-gameText"],
        true
    );
    Ok(())
}

fn property<'schema>(
    schema: &'schema Value,
    owner: &str,
    field: &str,
) -> TestResult<&'schema Value> {
    schema
        .get("$defs")
        .and_then(|definitions| definitions.get(owner))
        .and_then(|definition| definition.get("properties"))
        .and_then(|properties| properties.get(field))
        .ok_or_else(|| "generated owner-field schema is missing".into())
}

fn resolve<'schema>(
    root: &'schema Value,
    mut schema: &'schema Value,
) -> TestResult<&'schema Value> {
    let mut visited = BTreeSet::new();
    while let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        if !visited.insert(reference) {
            return Err("schema reference cycle".into());
        }
        let pointer = reference
            .strip_prefix('#')
            .ok_or("nonlocal schema reference")?;
        schema = root
            .pointer(pointer)
            .ok_or("closed generated schema reference")?;
    }
    Ok(schema)
}

fn present_value<'schema>(
    root: &'schema Value,
    schema: &'schema Value,
) -> TestResult<&'schema Value> {
    let carrier = resolve(root, schema)?;
    carrier
        .get("allOf")
        .and_then(Value::as_array)
        .and_then(|clauses| {
            clauses.iter().find(|clause| {
                clause
                    .pointer("/if/properties/state/const")
                    .and_then(Value::as_str)
                    == Some("present")
            })
        })
        .and_then(|clause| clause.pointer("/then/properties/value"))
        .ok_or_else(|| "present-state value schema is missing".into())
}

fn marked(root: &Value, schema: &Value, visited: &mut BTreeSet<String>) -> TestResult<bool> {
    if schema.get("x-gameText").and_then(Value::as_bool) == Some(true) {
        return Ok(true);
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        if !visited.insert(reference.to_owned()) {
            return Ok(false);
        }
        let pointer = reference
            .strip_prefix('#')
            .ok_or("nonlocal schema reference")?;
        if marked(
            root,
            root.pointer(pointer).ok_or("schema reference closure")?,
            visited,
        )? {
            return Ok(true);
        }
    }
    match schema {
        Value::Object(object) => {
            for child in object.values() {
                if marked(root, child, visited)? {
                    return Ok(true);
                }
            }
        }
        Value::Array(array) => {
            for child in array {
                if marked(root, child, visited)? {
                    return Ok(true);
                }
            }
        }
        _ => {}
    }
    Ok(false)
}
