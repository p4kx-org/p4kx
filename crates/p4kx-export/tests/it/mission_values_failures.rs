// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mission_values_fixture::{self as fixture, Owner, TestResult};
use p4kx_export::CatalogPublicationMode;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestStruct, TestType, TestValue};
use serde_json::Value;

#[test]
fn unsupported_search_clue_and_item_variants_fail_the_owner_and_block_normal_publication()
-> TestResult {
    let auto = TestInstance::new("MissionPropertyValue_AutoSpawns").value(
        "searchOptions",
        fixture::object(TestInstance::new("SpawnClosetSearchOptions_Unknown")),
    );
    let unknown_item = TestInstance::new("Investigation_ClueItem_Unknown");
    let unknown_clue = TestInstance::new("Investigation_ClueItem").value(
        "clues",
        fixture::array(vec![fixture::object(TestInstance::new(
            "Investigation_Clue_Unknown",
        ))]),
    );
    for value in [
        auto,
        investigation(unknown_item),
        investigation(unknown_clue),
    ] {
        verify_failure(value)?;
    }
    Ok(())
}

fn investigation(item: TestInstance) -> TestInstance {
    TestInstance::new("MissionPropertyValue_Investigation").value(
        "locations",
        fixture::array(vec![fixture::object(
            TestInstance::new("Investigation_Location")
                .value("clueItems", fixture::array(vec![fixture::object(item)])),
        )]),
    )
}

fn verify_failure(value: TestInstance) -> TestResult {
    let builder = fixture::database(
        vec![fixture::property("Unsupported", value)],
        Owner::Generator,
    );
    assert!(
        fixture::build(&builder, CatalogPublicationMode::Normal).is_err(),
        "unsupported authored source must block Normal"
    );
    let diagnostic = fixture::build(&builder, CatalogPublicationMode::DiagnosticPartial)?;
    assert_eq!(
        *fixture::field(&diagnostic, "/exportHealth/status")?,
        "incomplete",
        "explicit diagnostic publication retains incomplete health"
    );
    assert_eq!(
        fixture::field(&diagnostic, "/missionPropertySets")?
            .as_array()
            .ok_or("sets missing")?
            .len(),
        0,
        "an unsupported populated array is never presented as a successful empty set"
    );
    let failures = fixture::field(&diagnostic, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("diagnostics missing")?;
    assert!(
        failures
            .iter()
            .any(|failure| failure.get("code").and_then(Value::as_str) == Some("unsupportedValue")),
        "the exact unsupported category is machine-readable"
    );
    Ok(())
}

#[test]
fn an_unreadable_present_search_pointer_is_never_an_absent_configuration() -> TestResult {
    let bytes = (u32::MAX - 1)
        .to_le_bytes()
        .into_iter()
        .chain(3_u32.to_le_bytes())
        .collect();
    let builder = fixture::database(
        vec![fixture::property(
            "Broken",
            TestInstance::new("MissionPropertyValue_AutoSpawns")
                .value("searchOptions", TestValue::Raw(bytes)),
        )],
        Owner::Template,
    );
    assert!(
        fixture::build(&builder, CatalogPublicationMode::Normal).is_err(),
        "a present unreadable pointer blocks Normal"
    );
    let document = fixture::build(&builder, CatalogPublicationMode::DiagnosticPartial)?;
    assert_eq!(
        fixture::field(&document, "/contractTemplates")?
            .as_array()
            .ok_or("templates missing")?
            .len(),
        0,
        "failed fields must not become a null search configuration in a retained template"
    );
    assert_ne!(
        *fixture::field(&document, "/exportHealth/status")?,
        Value::String("complete".to_owned()),
        "canonical field failure remains visible"
    );
    Ok(())
}

#[test]
fn malformed_present_options_and_inexact_integer_sources_fail_instead_of_becoming_null()
-> TestResult {
    let values = [
        (
            TestStruct::new("MissionPropertyValue_Integer")
                .parent("BaseMissionPropertyValue")
                .field("options", TestType::Int32),
            TestInstance::new("MissionPropertyValue_Integer").value("options", TestValue::Int32(3)),
        ),
        (
            TestStruct::new("MissionPropertyValue_Integer")
                .parent("BaseMissionPropertyValue")
                .array(
                    "options",
                    TestType::Class("MissionPropertyValueOption_Integer".to_owned()),
                ),
            TestInstance::new("MissionPropertyValue_Integer").value(
                "options",
                fixture::array(vec![fixture::object(
                    TestInstance::new("MissionPropertyValueOption_Integer")
                        .value("value", TestValue::Single(3.5)),
                )]),
            ),
        ),
    ];
    for (structure, value) in values {
        let properties = fixture::array(vec![fixture::property("Malformed", value)]);
        let builder = crate::scoped_fixture::schema(DataForgeBuilder::new().version(8))
            .structure(structure)
            .structure(
                TestStruct::new("MissionPropertyValueOption_Integer")
                    .field("value", TestType::Single),
            )
            .record(crate::mission_property_sets_fixture::owner().value(
                "generators",
                fixture::array(vec![fixture::object(
                        TestInstance::new("ContractGeneratorHandler_Career").value(
                            "contractParams",
                            fixture::object(
                                TestInstance::new("ContractParamOverrides")
                                    .value("propertyOverrides", properties),
                            ),
                        ),
                    )]),
            ));
        assert!(
            fixture::build(&builder, CatalogPublicationMode::Normal).is_err(),
            "a wrong known field shape blocks Normal"
        );
        let document = fixture::build(&builder, CatalogPublicationMode::DiagnosticPartial)?;
        let diagnostics = fixture::field(&document, "/exportHealth/diagnostics")?
            .as_array()
            .ok_or("diagnostics missing")?;
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.get("code").and_then(Value::as_str)
                    == Some("malformedSource")),
            "a present malformed field is distinct from absent or unsupported data"
        );
    }
    Ok(())
}

#[test]
fn additional_declared_search_fields_are_not_silently_discarded() -> TestResult {
    let source = fixture::property(
        "Additional",
        TestInstance::new("MissionPropertyValue_AutoSpawns").value(
            "searchOptions",
            fixture::object(
                TestInstance::new("SpawnClosetSearchOptionsBase")
                    .value("futureField", TestValue::Bool(true)),
            ),
        ),
    );
    let generator = crate::mission_property_sets_fixture::generator(std::array::from_fn(|_| {
        crate::mission_property_sets_fixture::parameters(vec![source.clone()])
    }));
    let builder = crate::scoped_fixture::schema(DataForgeBuilder::new().version(8))
        .structure(
            TestStruct::new("MissionPropertyValue_AutoSpawns")
                .parent("BaseMissionPropertyValue")
                .field(
                    "searchOptions",
                    TestType::StrongPointer("SpawnClosetSearchOptionsBase".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("SpawnClosetSearchOptionsBase").field("futureField", TestType::Bool),
        )
        .record(generator);
    assert!(
        fixture::build(&builder, CatalogPublicationMode::Normal).is_err(),
        "unrepresented declared data blocks normal publication"
    );
    let document = fixture::build(&builder, CatalogPublicationMode::DiagnosticPartial)?;
    let diagnostics = fixture::field(&document, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("diagnostics missing")?;
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.get("code").and_then(Value::as_str)
                == Some("unsupportedValue")
                && diagnostic
                    .get("examples")
                    .and_then(Value::as_array)
                    .is_some_and(|examples| examples.iter().any(|example| example
                        .get("sourceType")
                        .and_then(Value::as_str)
                        == Some("SpawnClosetSearchOptionsBase")
                        && example.get("field").and_then(Value::as_str)
                            == Some("searchOptions.futureField")))),
        "the unknown source declaration has exact typed field evidence"
    );
    Ok(())
}
