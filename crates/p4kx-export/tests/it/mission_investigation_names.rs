// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Investigation names retain original tokens without changing established presentation keys.

use crate::mission_values_fixture::{self as fixture, Owner, TestResult};
use p4kx_export::CatalogPublicationMode;
use p4kx_testkit::{TestInstance, TestValue};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
enum Position {
    Suspect,
    Corpse,
}

impl Position {
    const fn path(self) -> &'static str {
        match self {
            Self::Suspect => "/suspects/0/name",
            Self::Corpse => "/locations/0/clueItems/0/clues/0/details/name",
        }
    }
}

fn name(given: TestValue, target: u128, chance: f32) -> TestInstance {
    TestInstance::new("MissionPropertyValue_AIName")
        .value("randomName", TestValue::Bool(false))
        .value("randomLastName", TestValue::Bool(false))
        .value("randomNickName", TestValue::Bool(false))
        .value("characterGivenName", given)
        .value("characterGivenLastName", TestValue::text(""))
        .value(
            "characterGivenNickName",
            TestValue::text("@LOC_UNINITIALIZED"),
        )
        .value("characterNameData", TestValue::Reference(target))
        .value("chanceOfNickName", TestValue::Single(chance))
}

fn investigation(name: TestInstance, position: Position) -> TestInstance {
    let value = TestInstance::new("MissionPropertyValue_Investigation");
    match position {
        Position::Suspect => value.value(
            "suspects",
            fixture::array(vec![fixture::object(
                TestInstance::new("Investigation_Suspect").value("name", fixture::object(name)),
            )]),
        ),
        Position::Corpse => value.value(
            "locations",
            fixture::array(vec![fixture::object(
                TestInstance::new("Investigation_Location").value(
                    "clueItems",
                    fixture::array(vec![fixture::object(
                        TestInstance::new("Investigation_ClueItem").value(
                            "clues",
                            fixture::array(vec![fixture::object(
                                TestInstance::new("Investigation_Clue_CorpseScanInfo")
                                    .value("name", fixture::object(name)),
                            )]),
                        ),
                    )]),
                ),
            )]),
        ),
    }
}

fn build_name(
    name: TestInstance,
    position: Position,
    owner: Owner,
    mode: CatalogPublicationMode,
) -> Result<Value, Box<dyn std::error::Error>> {
    fixture::build(
        &fixture::database(
            vec![fixture::property("Case", investigation(name, position))],
            owner,
        ),
        mode,
    )
}

#[test]
fn original_name_tokens_and_dangling_targets_survive_both_investigation_positions() -> TestResult {
    for owner in [Owner::Generator, Owner::Template, Owner::Broker] {
        for position in [Position::Suspect, Position::Corpse] {
            let document = build_name(
                name(TestValue::text("plain"), fixture::MISSING, 0.0),
                position,
                owner,
                CatalogPublicationMode::Normal,
            )?;
            let value = fixture::value(&document, owner, "Case")?
                .pointer(position.path())
                .ok_or("the investigation name")?;
            assert_eq!(value["source"]["characterGivenName"], "plain");
            assert_eq!(value["source"]["characterGivenLastName"], "");
            assert_eq!(
                value["source"]["characterGivenNickName"],
                "@LOC_UNINITIALIZED"
            );
            for text in ["givenName", "givenLastName", "givenNickName"] {
                assert_eq!(value[text]["key"], Value::Null);
            }
            let reference = &value["source"]["characterNameData"];
            assert_eq!(reference["status"], "dangling");
            assert_eq!(
                reference["guid"],
                crate::catalog_crafting_support::text(fixture::MISSING)
            );
            assert_eq!(reference["targetRecordIndex"], Value::Null);
            assert_eq!(value["nameDataGuid"], Value::Null);
            assert_eq!(value["chanceOfNickName"], 0.0);
            assert_eq!(value["randomName"], false);
        }
    }
    Ok(())
}

#[test]
fn resolved_name_data_keeps_its_full_reference_and_the_legacy_guid() -> TestResult {
    for position in [Position::Suspect, Position::Corpse] {
        let document = build_name(
            name(TestValue::text("@given"), fixture::TARGET, 0.25),
            position,
            Owner::Generator,
            CatalogPublicationMode::Normal,
        )?;
        let value = fixture::value(&document, Owner::Generator, "Case")?
            .pointer(position.path())
            .ok_or("the investigation name")?;
        let reference = &value["source"]["characterNameData"];
        assert_eq!(reference["status"], "present");
        assert!(reference["targetRecordIndex"].is_u64());
        assert_eq!(reference["recordPath"], "synthetic/definition");
        assert_eq!(reference["targetType"], "Definition");
        assert_eq!(reference["guid"], value["nameDataGuid"]);
        assert_eq!(value["givenName"]["key"], "@given");
    }
    Ok(())
}

#[test]
fn malformed_present_name_tokens_and_nonfinite_chances_fail_their_original_owner() -> TestResult {
    for position in [Position::Suspect, Position::Corpse] {
        let malformed = TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec());
        for (name, field) in [
            (name(malformed, fixture::MISSING, 0.0), "characterGivenName"),
            (
                name(TestValue::text("@given"), fixture::MISSING, f32::NAN),
                "chanceOfNickName",
            ),
        ] {
            assert!(
                build_name(
                    name.clone(),
                    position,
                    Owner::Generator,
                    CatalogPublicationMode::Normal
                )
                .is_err()
            );
            let document = build_name(
                name,
                position,
                Owner::Generator,
                CatalogPublicationMode::DiagnosticPartial,
            )?;
            assert_eq!(document["exportHealth"]["status"], "incomplete");
            assert_eq!(document["missionPropertySets"], json!([]));
            let failures = document["exportHealth"]["diagnostics"]
                .as_array()
                .ok_or("the diagnostics")?;
            assert!(
                failures
                    .iter()
                    .any(|failure| failure["examples"]
                        .as_array()
                        .is_some_and(|examples| examples.iter().any(
                            |example| example["sourceType"] == "MissionPropertyValue_AIName"
                                && example["field"]
                                    .as_str()
                                    .is_some_and(|path| path.ends_with(&format!(".name.{field}")))
                        )))
            );
        }
    }
    Ok(())
}

#[test]
fn investigation_source_additions_leave_legacy_ai_name_properties_unchanged() -> TestResult {
    let document = fixture::build(
        &fixture::database(
            vec![fixture::property(
                "Legacy",
                name(TestValue::text("plain"), fixture::MISSING, 0.0),
            )],
            Owner::Generator,
        ),
        CatalogPublicationMode::Normal,
    )?;
    let value = fixture::value(&document, Owner::Generator, "Legacy")?;
    assert_eq!(value["type"], "aiName");
    assert_eq!(value["givenName"]["key"], Value::Null);
    assert_eq!(value["nameDataGuid"], Value::Null);
    assert!(value.get("source").is_none());
    Ok(())
}

#[test]
fn the_name_schema_requires_original_source_fields_even_when_null() -> TestResult {
    let document = build_name(
        name(TestValue::text("plain"), fixture::MISSING, 0.0),
        Position::Suspect,
        Owner::Generator,
        CatalogPublicationMode::Normal,
    )?;
    let mut value = fixture::value(&document, Owner::Generator, "Case")?
        .pointer(Position::Suspect.path())
        .ok_or("the investigation name")?
        .clone();
    let schema = p4kx_export::catalog_schema();
    let name_schema = json!({"$schema": schema["$schema"], "$defs": schema["$defs"], "$ref": "#/$defs/InvestigationName"});
    let validator = jsonschema::validator_for(&name_schema)?;
    assert!(validator.is_valid(&value));
    value["source"]
        .as_object_mut()
        .ok_or("the source object")?
        .remove("characterGivenName");
    assert!(!validator.is_valid(&value));
    Ok(())
}
