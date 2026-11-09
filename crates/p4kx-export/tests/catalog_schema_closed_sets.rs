// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Catalog source vocabularies are extensible; fixed control states are closed.

#![recursion_limit = "256"]

mod catalog_schema_support;

use catalog_schema_support::minimal_catalog;
use serde_json::{Map, Value, json};

const VOCABULARY_CLASS: &str = "x-p4kx-vocabularyClass";
const CONTROL_TYPES: &[&str] = &[
    "CatalogPublicationMode",
    "CommoditySelection",
    "DatasetAvailability",
    "DefinitionStatus",
    "ExportDiagnosticSeverity",
    "ExportDiagnosticStage",
    "ExportDiagnosticUnit",
    "ExportHealthStatus",
    "ExportSourceMode",
    "ExportSourceRole",
    "ExportStageStatus",
    "InputScope",
    "OwnedReferenceStatus",
    "PropertyReferenceStatus",
    "QuantizationIneligibility",
    "SourceCoordinateOmission",
    "SourceDeclarationKind",
    "SourceDetails",
    "SourceEncryptionState",
    "SourceRecordFormat",
    "SourceUseRole",
    "SourceValueState",
];
const OPEN_TYPES: &[&str] = &[
    "ArchiveKeyState",
    "BlueprintSource",
    "CargoFillType",
    "CargoSpawnKind",
    "CommsTiming",
    "Comparison",
    "EndReason",
    "EntryOperation",
    "ExportDiagnosticCode",
    "ExportReferenceStatus",
    "FactLayer",
    "GiverSource",
    "HaulingItemKind",
    "HaulingSource",
    "Kind",
    "LicenseType",
    "LocationCount",
    "MissionPropertyLevel",
    "Month",
    "ObjectiveCategory",
    "ObjectiveState",
    "PropertyKind",
    "Reaction",
    "RepeatType",
    "SpawnLevel",
    "StandingLevel",
];

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn has_finite_set(object: &Map<String, Value>) -> bool {
    object.contains_key("enum")
        || ["oneOf", "anyOf"].into_iter().any(|keyword| {
            object
                .get(keyword)
                .and_then(Value::as_array)
                .is_some_and(|variants| {
                    !variants.is_empty()
                        && variants
                            .iter()
                            .all(|variant| variant.get("const").is_some())
                })
        })
}

fn vocabulary_class_violation(object: &Map<String, Value>) -> Option<&'static str> {
    let finite = has_finite_set(object);
    match object.get(VOCABULARY_CLASS) {
        Some(Value::String(class)) if class == "control" => {
            (!finite).then_some("control requires finite alternatives")
        }
        Some(Value::String(class)) if class == "open" => (finite
            || object.get("type").and_then(Value::as_str) != Some("string")
            || !object.get("x-values").is_some_and(Value::is_array))
        .then_some("open vocabulary requires string with x-values"),
        Some(_) => Some("unknown vocabulary class"),
        None => finite.then_some("finite scalar set requires control class"),
    }
}

fn vocabulary_policy_violations(node: &Value, pointer: &str, found: &mut Vec<String>) {
    match node {
        Value::Object(object) => {
            if let Some(reason) = vocabulary_class_violation(object) {
                found.push(format!("{pointer}: {reason}"));
            }
            for (key, child) in object {
                vocabulary_policy_violations(child, &format!("{pointer}/{key}"), found);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                vocabulary_policy_violations(child, &format!("{pointer}/{index}"), found);
            }
        }
        _ => {}
    }
}

#[test]
fn scalar_value_sets_have_a_deliberate_schema_class() {
    let schema = p4kx_export::catalog_schema();
    let mut found = Vec::new();
    vocabulary_policy_violations(&schema, "", &mut found);
    assert!(found.is_empty(), "vocabulary policy violations: {found:?}");

    let definitions = schema["$defs"].as_object().expect("schema definitions");
    for (class, expected) in [("control", CONTROL_TYPES), ("open", OPEN_TYPES)] {
        let mut actual: Vec<_> = definitions
            .iter()
            .filter(|(_, definition)| definition[VOCABULARY_CLASS] == class)
            .map(|(name, _)| name.as_str())
            .collect();
        actual.sort_unstable();
        assert_eq!(actual, expected, "explicit {class} vocabulary inventory");
    }
}

#[test]
fn policy_checks_reject_unclassified_or_misclassified_finite_sets() {
    let schema = json!({
        "$defs": {
            "Unclassified": {"enum": ["x"]},
            "UnclassifiedOneOf": {"oneOf": [{"const": "x"}, {"const": "y"}]},
            "UnclassifiedAnyOf": {"anyOf": [{"const": "x"}]},
            "OpenFinite": {"x-p4kx-vocabularyClass": "open", "enum": ["x"]},
            "ControlOpen": {"x-p4kx-vocabularyClass": "control", "type": "string"},
            "UnknownClass": {"x-p4kx-vocabularyClass": "futureClass", "type": "string"},
            "ValidControl": {"x-p4kx-vocabularyClass": "control", "enum": ["x"]},
            "ValidOpen": {"x-p4kx-vocabularyClass": "open", "type": "string", "x-values": [{"value": "x"}]},
            "Payload": {"oneOf": [{"const": "x"}, {"type": "object"}]}
        }
    });
    let mut found = Vec::new();
    vocabulary_policy_violations(&schema, "", &mut found);
    assert_eq!(found.len(), 6, "each invalid class is found: {found:?}");
    assert!(
        found.iter().all(|entry| !entry.contains("Valid")),
        "valid classes: {found:?}"
    );
    assert!(
        found.iter().all(|entry| !entry.contains("Payload")),
        "payload union: {found:?}"
    );
}

#[test]
fn every_open_vocabulary_accepts_known_and_unknown_strings() -> TestResult {
    let schema = p4kx_export::catalog_schema();
    for name in OPEN_TYPES {
        let definition = &schema["$defs"][name];
        assert_eq!(definition[VOCABULARY_CLASS], "open", "{name}");
        let values = definition["x-values"].as_array().expect("known values");
        assert!(!values.is_empty(), "{name} documents its known values");
        let validator = jsonschema::validator_for(definition)?;
        for entry in values {
            assert!(validator.is_valid(&entry["value"]), "{name}: {entry}");
        }
        assert!(
            validator.is_valid(&json!("futureSourceValue")),
            "{name} accepts an unknown string"
        );
        for invalid in [json!(42), json!(true), json!({}), json!([]), Value::Null] {
            assert!(!validator.is_valid(&invalid), "{name} rejects {invalid}");
        }
    }
    Ok(())
}

#[test]
fn catalog_health_rejects_unknown_control_states() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let catalog = minimal_catalog();
    assert!(validator.is_valid(&catalog), "the minimal catalog is valid");
    for pointer in [
        "/exportHealth/status",
        "/exportHealth/read",
        "/exportHealth/projection",
        "/exportHealth/publicationMode",
        "/exportHealth/kinds/0/sourceMode",
        "/exportHealth/kinds/0/read",
        "/exportHealth/kinds/0/projection",
        "/exportHealth/diagnostics/0/stage",
        "/exportHealth/diagnostics/0/severity",
        "/exportHealth/diagnostics/0/sourceRole",
        "/exportHealth/diagnostics/0/countUnit",
    ] {
        let mut unknown = catalog.clone();
        *unknown
            .pointer_mut(pointer)
            .expect("a populated fixture control") = json!("futureControlState");
        assert!(
            !validator.is_valid(&unknown),
            "{pointer} rejects an unknown control"
        );
    }
    Ok(())
}

#[test]
fn a_new_source_annotation_value_is_additive() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let catalog = minimal_catalog();
    assert!(validator.is_valid(&catalog), "the minimal catalog is valid");

    let mut new_layer = catalog.clone();
    new_layer["contracts"][0]["canBeShared"]["layer"] = json!("newLayer");
    new_layer["contracts"][0]["cooldownLayer"] = json!("newLayer");
    assert!(validator.is_valid(&new_layer), "a new fact layer is valid");

    let mut new_level = catalog.clone();
    new_level["contracts"][0]["standingRequirements"][0]["level"] = json!("newLevel");
    new_level["contracts"][0]["requiredIntros"][0]["level"] = json!("newLevel");
    new_level["contracts"][0]["shipSpawns"][0]["level"] = json!("newLevel");
    assert!(
        validator.is_valid(&new_level),
        "a new standing or spawn level is valid"
    );

    let mut layer_number = catalog;
    layer_number["contracts"][0]["canBeShared"]["layer"] = json!(3);
    assert!(
        !validator.is_valid(&layer_number),
        "a fact layer stays a string"
    );
    Ok(())
}

#[test]
fn every_control_vocabulary_rejects_unknown_and_non_string_values() -> TestResult {
    let schema = p4kx_export::catalog_schema();
    for name in CONTROL_TYPES {
        let validator = jsonschema::validator_for(&schema["$defs"][name])?;
        for invalid in [
            json!("futureControlState"),
            json!(0),
            json!(null),
            json!({}),
            json!([]),
        ] {
            assert!(
                !validator.is_valid(&invalid),
                "{name} rejects an invalid control: {invalid}"
            );
        }
    }
    Ok(())
}
