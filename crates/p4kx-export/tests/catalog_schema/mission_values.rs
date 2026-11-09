// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::{Value, json};

use crate::catalog_schema_support;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const CONTRACT_PROPERTY: &str = "/contractTemplates/0/contractProperties/0";
const QUALITY_DISTRIBUTION: &str =
    "/definitions/qualityDistributions/rows/0/qualityDistribution/value/payload/value/value/value";

fn with_value(value: &Value) -> TestResult<Value> {
    let mut catalog = catalog_schema_support::minimal_catalog();
    *catalog
        .pointer_mut(CONTRACT_PROPERTY)
        .ok_or("contract property fixture absent")? = json!({
        "variable": "Schema_BP", "token": null, "valueType": null, "value": value
    });
    Ok(catalog)
}

fn option(value: &Value, variation: &Value) -> Value {
    json!({
        "displayName": {"key": null, "en": null, "de": null},
        "textId": null, "value": value, "variation": variation, "weighting": 0.125,
        "debugForceChooseThisOption": false,
        "dependentProperties": [null, {"property": null, "values": [null]}]
    })
}

fn option_mut(value: &mut Value, index: usize) -> TestResult<&mut Value> {
    value
        .get_mut("options")
        .and_then(Value::as_array_mut)
        .and_then(|options| options.get_mut(index))
        .ok_or_else(|| "option fixture absent".into())
}

fn replace(value: &mut Value, field: &str, replacement: Value) -> TestResult {
    *value.get_mut(field).ok_or("fixture field absent")? = replacement;
    Ok(())
}

fn invalid_integers() -> [Value; 3] {
    [
        json!(1.5),
        json!(i64::from(i32::MAX) + 1),
        json!(i64::from(i32::MIN) - 1),
    ]
}

#[test]
fn integer_options_require_exact_i32_values_and_keep_fractional_weights() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let value =
        json!({"type": "integer", "options": [null, option(&json!(i32::MAX), &json!(i32::MIN))]});
    assert!(
        validator.is_valid(&with_value(&value)?),
        "full signed source range is valid"
    );
    for invalid in invalid_integers() {
        for field in ["value", "variation"] {
            let mut changed = value.clone();
            replace(option_mut(&mut changed, 1)?, field, invalid.clone())?;
            assert!(
                !validator.is_valid(&with_value(&changed)?),
                "integer source fields cannot become fractional or overflow"
            );
        }
    }
    Ok(())
}

#[test]
fn nullable_option_arrays_and_nodes_remain_explicit_and_structured() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for kind in ["integer", "float", "stringHash"] {
        for options in [Value::Null, json!([]), json!([null])] {
            assert!(
                validator.is_valid(&with_value(&json!({"type": kind, "options": options}))?),
                "missing array differs from empty array and null nodes"
            );
        }
    }
    let mut value = json!({"type": "float", "options": [option(&json!(0.25), &json!(0.125))]});
    assert!(
        validator.is_valid(&with_value(&value)?),
        "structured dependencies and fractional values validate"
    );
    replace(
        option_mut(&mut value, 0)?,
        "dependentProperties",
        json!(["Schema_BP"]),
    )?;
    assert!(
        !validator.is_valid(&with_value(&value)?),
        "dependency nodes cannot collapse to variable-name strings"
    );
    Ok(())
}

#[test]
fn newly_declared_owned_variants_validate_without_runtime_results() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let searches = [
        json!({"type": "base"}),
        json!({"type": "entityDeclaration", "entityDeclaration": null}),
        json!({"type": "tags", "closetPositiveTags": null, "closetNegativeTags": {"tags": [null]}}),
    ];
    for search in searches {
        let value = json!({"type": "autoSpawns", "autoSpawns": null, "searchOptions": search});
        assert!(
            validator.is_valid(&with_value(&value)?),
            "every declared closet-search variant is represented"
        );
    }
    assert!(
        validator.is_valid(&with_value(&investigation(&Value::Null))?),
        "authored investigation arrays, nullable names and null positions validate"
    );
    let race = json!({"type": "timeTrialRace", "targetSplits": [null, {"targetSplit": 0.125}]});
    assert!(
        validator.is_valid(&with_value(&race)?),
        "authored race splits retain their source precision"
    );
    Ok(())
}

#[test]
fn newly_nullable_payload_fields_are_required_even_when_unknown() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for value in [
        json!({"type": "autoSpawns"}),
        json!({"type": "investigation"}),
        json!({"type": "timeTrialRace"}),
        json!({"type": "tags", "tags": [], "negativeTags": []}),
    ] {
        assert!(
            !validator.is_valid(&with_value(&value)?),
            "a typed source variant cannot silently omit its new fields"
        );
    }
    for field in ["debugForceChooseThisOption", "textId"] {
        let mut missing = option(&json!(1), &json!(0));
        missing
            .as_object_mut()
            .ok_or("option fixture is not an object")?
            .remove(field);
        assert!(
            !validator.is_valid(&with_value(
                &json!({"type": "integer", "options": [missing]})
            )?),
            "nullable option metadata and raw textId stay required"
        );
    }
    Ok(())
}

#[test]
fn raw_option_text_ids_preserve_empty_placeholder_and_null_tokens() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for text_id in [Value::Null, json!(""), json!("@LOC_UNINITIALIZED")] {
        let mut value = option(&json!(1), &json!(0));
        replace(&mut value, "textId", text_id)?;
        assert!(
            validator.is_valid(&with_value(
                &json!({"type": "integer", "options": [value]})
            )?),
            "raw option textId availability does not depend on presentation filtering"
        );
    }
    Ok(())
}

#[test]
fn raw_quality_bounds_validate_the_exact_declared_number_tag_and_storage() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let mut catalog = catalog_schema_support::minimal_catalog();
    for (field, value) in [("min", i32::MIN), ("max", i32::MAX)] {
        *catalog
            .pointer_mut(&format!("{QUALITY_DISTRIBUTION}/{field}/value"))
            .ok_or("quality numeric token")? = json!({"type":"int32", "value":value});
    }
    assert!(
        validator.is_valid(&catalog),
        "the complete signed Int32 payload range remains exact"
    );
    for invalid in invalid_integers() {
        for field in ["min", "max"] {
            let mut changed = catalog.clone();
            *changed
                .pointer_mut(&format!("{QUALITY_DISTRIBUTION}/{field}/value/value"))
                .ok_or("quality Int32 payload")? = invalid.clone();
            assert!(
                !validator.is_valid(&changed),
                "an Int32-tagged raw payload cannot be fractional or overflow"
            );
        }
    }
    *catalog
        .pointer_mut(&format!("{QUALITY_DISTRIBUTION}/min/value"))
        .ok_or("quality numeric token")? =
        json!({"type":"f64", "rawBits":"3ff4000000000000", "finite":1.25});
    assert!(
        validator.is_valid(&catalog),
        "a wrong-contract numeric token can still be represented losslessly; export health diagnoses its declared Int32 contract"
    );
    Ok(())
}

fn investigation(name: &Value) -> Value {
    json!({
        "type": "investigation", "startingJournalEntries": [null],
        "suspects": [null, {"name": name, "initialSuspect": false, "isGuilty": false}],
        "locations": [null, {"clueItems": [null]}]
    })
}

fn investigation_name() -> Value {
    json!({
        "randomName": false, "randomLastName": true, "randomNickName": false,
        "givenName": {"key": null, "en": null, "de": null},
        "givenLastName": {"key": null, "en": null, "de": null},
        "givenNickName": {"key": null, "en": null, "de": null},
        "chanceOfNickName": 0.125,
        "nameDataGuid": "a9000000-0000-4000-8000-000000000001",
        "source": {
            "characterGivenName": "",
            "characterGivenLastName": "@LOC_UNINITIALIZED",
            "characterGivenNickName": null,
            "characterNameData": {
                "targetRecordIndex": 7,
                "guid": "a9000000-0000-4000-8000-000000000001",
                "recordPath": "synthetic/name_data",
                "targetType": "CharacterRandomNameParams", "status": "present"
            }
        }
    })
}

#[test]
fn investigation_names_keep_flattened_presentation_and_complete_raw_source() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let name = investigation_name();
    assert!(
        name.get("givenName").is_some() && name.get("name").is_none(),
        "established presentation fields remain directly on the investigation name"
    );
    assert!(
        validator.is_valid(&with_value(&investigation(&name))?),
        "raw empty and placeholder Locale tokens plus complete target identity validate"
    );
    let mut nullable = name;
    let source = nullable.get_mut("source").ok_or("name source absent")?;
    for field in [
        "characterGivenName",
        "characterGivenLastName",
        "characterGivenNickName",
        "characterNameData",
    ] {
        replace(source, field, Value::Null)?;
    }
    replace(&mut nullable, "nameDataGuid", Value::Null)?;
    assert!(
        validator.is_valid(&with_value(&investigation(&nullable))?),
        "source members stay explicitly nullable without making source itself optional"
    );
    Ok(())
}

#[test]
fn investigation_names_require_source_members_and_flattened_presentation() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for field in ["source", "givenName", "givenLastName", "givenNickName"] {
        let mut name = investigation_name();
        name.as_object_mut()
            .ok_or("name fixture is not an object")?
            .remove(field);
        assert!(
            !validator.is_valid(&with_value(&investigation(&name))?),
            "raw source and existing flattened text fields cannot disappear"
        );
    }
    for field in [
        "characterGivenName",
        "characterGivenLastName",
        "characterGivenNickName",
        "characterNameData",
    ] {
        let mut name = investigation_name();
        name.get_mut("source")
            .and_then(Value::as_object_mut)
            .ok_or("source fixture absent")?
            .remove(field);
        assert!(
            !validator.is_valid(&with_value(&investigation(&name))?),
            "every raw source member remains required when nullable"
        );
    }
    let mut presentation = investigation_name();
    let source = presentation
        .as_object_mut()
        .ok_or("name fixture absent")?
        .remove("source")
        .ok_or("source fixture absent")?;
    let nested = json!({"name": presentation, "source": source});
    assert!(
        !validator.is_valid(&with_value(&investigation(&nested))?),
        "presentation fields cannot move into an additional nested name object"
    );
    Ok(())
}

#[test]
fn investigation_name_data_requires_full_definition_reference_shape() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for field in [
        "targetRecordIndex",
        "guid",
        "recordPath",
        "targetType",
        "status",
    ] {
        let mut name = investigation_name();
        name.pointer_mut("/source/characterNameData")
            .and_then(Value::as_object_mut)
            .ok_or("name-data fixture absent")?
            .remove(field);
        assert!(
            !validator.is_valid(&with_value(&investigation(&name))?),
            "name-data links retain every DefinitionReference field"
        );
    }
    for (field, invalid) in [
        ("targetRecordIndex", json!(-1)),
        ("guid", json!("not-a-guid")),
        ("recordPath", json!(false)),
        ("targetType", json!(0)),
        ("status", json!(false)),
    ] {
        let mut name = investigation_name();
        replace(
            name.pointer_mut("/source/characterNameData")
                .ok_or("name-data fixture absent")?,
            field,
            invalid,
        )?;
        assert!(
            !validator.is_valid(&with_value(&investigation(&name))?),
            "DefinitionReference identities keep their typed shape"
        );
    }
    let mut dangling = investigation_name();
    let reference = dangling
        .pointer_mut("/source/characterNameData")
        .ok_or("name-data fixture absent")?;
    for field in ["targetRecordIndex", "recordPath", "targetType"] {
        replace(reference, field, Value::Null)?;
    }
    replace(reference, "status", json!("dangling"))?;
    replace(&mut dangling, "nameDataGuid", Value::Null)?;
    assert!(
        validator.is_valid(&with_value(&investigation(&dangling))?),
        "a dangling name-data link retains its GUID with explicitly unknown physical metadata"
    );
    Ok(())
}

#[test]
fn investigation_sources_reject_non_objects_and_non_text_tokens() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for path in [
        "/source",
        "/source/characterGivenName",
        "/source/characterGivenLastName",
        "/source/characterGivenNickName",
        "/source/characterNameData",
    ] {
        let mut name = investigation_name();
        *name.pointer_mut(path).ok_or("name source fixture absent")? = json!(false);
        assert!(
            !validator.is_valid(&with_value(&investigation(&name))?),
            "raw Locale tokens and source reference keep their declared types"
        );
    }
    let mut name = investigation_name();
    replace(&mut name, "source", Value::Null)?;
    assert!(
        !validator.is_valid(&with_value(&investigation(&name))?),
        "nullable source members do not make the source object nullable"
    );
    Ok(())
}
