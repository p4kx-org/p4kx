// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Investigation names retain original tokens and provenance for their protected presentation.

use crate::mission_values_fixture::{self as fixture, Owner, TestResult};
use p4kx_export::CatalogPublicationMode;
use p4kx_extraction::Language;
use p4kx_testkit::{TestEntry, TestInstance, TestValue};
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::mission_canonical_health as canonical_health;

#[path = "mission_name_canonical.rs"]
mod canonical;

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
            if !matches!(owner, Owner::Generator) {
                canonical::original(&document, position, owner)?;
                continue;
            }
            let value = fixture::generator_value(&document, "Case")?
                .pointer(position.path())
                .ok_or("the investigation name")?;
            assert_eq!(value["source"]["characterGivenName"], "plain");
            assert_eq!(value["source"]["characterGivenLastName"], "");
            assert_eq!(
                value["source"]["characterGivenNickName"],
                "@LOC_UNINITIALIZED"
            );
            for text in ["givenName", "givenLastName", "givenNickName"] {
                proof::cell(value, text, "absent")?;
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
        let value = fixture::generator_value(&document, "Case")?
            .pointer(position.path())
            .ok_or("the investigation name")?;
        let reference = &value["source"]["characterNameData"];
        assert_eq!(reference["status"], "present");
        assert!(reference["targetRecordIndex"].is_u64());
        assert_eq!(reference["recordPath"], "synthetic/definition");
        assert_eq!(reference["targetType"], "Definition");
        assert_eq!(reference["guid"], value["nameDataGuid"]);
        let target = proof::present(value, "givenName", &json!("text:6:@given"), Some(false))?;
        assert_eq!(
            target["sourceSetComplete"], true,
            "the exact original name field is covered"
        );
        assert_eq!(
            value["source"]["characterGivenName"], "@given",
            "the original Locale token remains available"
        );
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
            assert!(
                canonical_health::example(
                    &document,
                    None,
                    "MissionPropertyValue_AIName",
                    |path| path.ends_with(&format!(".name.{field}"))
                )?,
                "the same report retains the original failing name field and source type"
            );
        }
    }
    Ok(())
}

#[test]
fn generator_ai_names_retain_their_lookup_policy_and_original_field_evidence() -> TestResult {
    let database = fixture::database(
        vec![
            fixture::property(
                "Given",
                name(TestValue::text("@given"), fixture::TARGET, 0.25),
            ),
            fixture::property(
                "Bare",
                name(TestValue::text("plain"), fixture::MISSING, 0.0),
            ),
        ],
        Owner::Generator,
    );
    let english = TestEntry::stored(
        Language::English.global_ini_path(),
        b"given=Given name\nplain=Not a name key\nLOC_UNINITIALIZED=Placeholder\n",
    );
    let catalog =
        fixture::build_with_entries(&database, CatalogPublicationMode::Normal, [english])?;
    let value = fixture::generator_value(&catalog, "Given")?;
    verify_direct_name_settings(value);
    verify_direct_name_field_evidence(&catalog, value)?;

    let bare = fixture::generator_value(&catalog, "Bare")?;
    assert_eq!(
        bare["nameDataGuid"],
        Value::Null,
        "the unresolved name data stays null"
    );
    proof::cell(bare, "givenName", "absent")?;
    assert!(
        proof::rows(&catalog, "/texts/gameTexts")?
            .iter()
            .all(|row| {
                !matches!(
                    row.pointer("/key/value").and_then(Value::as_str),
                    Some("plain" | "" | "@LOC_UNINITIALIZED")
                )
            }),
        "discarded name tokens do not register protected text",
    );
    proof::closure(&catalog)
}

fn verify_direct_name_settings(value: &Value) {
    assert_eq!(
        value["type"], "aiName",
        "the direct property tag stays stable"
    );
    assert_eq!(
        value["randomName"], false,
        "an authored false flag stays false"
    );
    assert_eq!(
        value["chanceOfNickName"], 0.25,
        "the exact nickname chance stays intact"
    );
    assert_eq!(
        value["nameDataGuid"],
        crate::catalog_crafting_support::text(fixture::TARGET),
        "the resolved name data retains its identity",
    );
    assert!(
        value.get("source").is_none(),
        "the direct variant has no second name payload"
    );
}

fn verify_direct_name_field_evidence(catalog: &Value, value: &Value) -> TestResult {
    let reference = proof::present(value, "givenName", &json!("text:6:@given"), Some(false))?;
    let row = crate::commodity_v3_fixture::text_row(catalog, "gameTexts", &reference["value"])?;
    assert_eq!(
        row.pointer("/translations/value/en/displayText/value"),
        Some(&json!("Given name")),
        "the reference resolves to the original English display text",
    );
    let uses = proof::uses(catalog, reference)?;
    assert!(
        uses.iter().any(|used| {
            used["role"] == "payload"
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && used.pointer("/location/indices/1").is_some()
        }),
        "the reference identifies the original Generator Locale field",
    );

    for field in ["givenLastName", "givenNickName"] {
        let absent = proof::cell(value, field, "absent")?;
        assert_eq!(
            absent["containsEncryptedSource"], false,
            "the absent decision has clear source evidence"
        );
        assert_ne!(
            absent["sourceUsesRefs"], reference["sourceUsesRefs"],
            "each name field carries its own location"
        );
    }
    Ok(())
}

#[test]
fn ambiguous_generator_ai_name_is_quarantined_before_text_publication() -> TestResult {
    use crate::mission_property_sets_fixture::{generator, parameters};

    let property =
        |key| fixture::property("Name", name(TestValue::text(key), fixture::TARGET, 0.25));
    let database =
        fixture::database(vec![property("@retained")], Owner::Generator).record(generator([
            parameters(vec![property("@discarded")]),
            parameters(vec![property("@retained")]),
            parameters(vec![property("@retained")]),
        ]));
    let catalog = fixture::build(&database, CatalogPublicationMode::DiagnosticPartial)?;
    let sets = proof::rows(&catalog, "/missionPropertySets")?;
    assert_eq!(
        sets.len(),
        3,
        "the other three property sets remain available"
    );
    for set in sets {
        assert_ne!(set["level"], "handler", "the ambiguous handler is withheld");
        let value = fixture::field(set, "/properties/0/value")?;
        proof::present(value, "givenName", &json!("text:9:@retained"), Some(false))?;
    }
    assert!(
        proof::rows(&catalog, "/texts/gameTexts")?
            .iter()
            .all(|row| row.pointer("/key/value").and_then(Value::as_str) != Some("@discarded")),
        "the discarded handler cannot register a text",
    );
    proof::closure(&catalog)
}

#[test]
fn different_raw_investigation_name_tokens_keep_original_aliases_distinct() -> TestResult {
    use crate::mission_property_sets_fixture::{generator, parameters};

    let property = |key| {
        fixture::property(
            "Case",
            investigation(
                name(TestValue::text(key), fixture::TARGET, 0.25),
                Position::Suspect,
            ),
        )
    };
    let database =
        fixture::database(vec![property("first")], Owner::Generator).record(generator([
            parameters(vec![property("second")]),
            parameters(vec![property("first")]),
            parameters(vec![property("first")]),
        ]));
    let catalog = fixture::build(&database, CatalogPublicationMode::DiagnosticPartial)?;
    let sets = proof::rows(&catalog, "/missionPropertySets")?;
    assert_eq!(sets.len(), 3, "three unambiguous source locations remain");
    for set in sets {
        assert_ne!(
            set["level"], "handler",
            "different raw keys quarantine the shared handler"
        );
        let name = fixture::field(set, "/properties/0/value/suspects/0/name")?;
        assert_eq!(
            name["source"]["characterGivenName"], "first",
            "the retained alias supplies its original token"
        );
        proof::cell(name, "givenName", "absent")?;
    }
    proof::closure(&catalog)
}

#[test]
fn the_name_schema_requires_original_source_fields_even_when_null() -> TestResult {
    let document = build_name(
        name(TestValue::text("plain"), fixture::MISSING, 0.0),
        Position::Suspect,
        Owner::Generator,
        CatalogPublicationMode::Normal,
    )?;
    let mut value = fixture::generator_value(&document, "Case")?
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
