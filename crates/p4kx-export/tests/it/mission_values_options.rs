// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mission_values_fixture::{self as fixture, Owner, TestResult};
use p4kx_export::CatalogPublicationMode;
use p4kx_testkit::{TestInstance, TestValue};
use serde_json::{Value, json};

fn option(kind: &str) -> TestInstance {
    TestInstance::new(format!("MissionPropertyValueOption_{kind}"))
        .value("textId", TestValue::text("@option"))
        .value("weighting", TestValue::Single(0.375))
        .value("DEBUG_forceChooseThisOption", TestValue::Bool(false))
}

fn declaration(kind: &str, options: Vec<TestValue>) -> TestValue {
    fixture::property(
        kind,
        TestInstance::new(format!("MissionPropertyValue_{kind}"))
            .value("options", fixture::array(options)),
    )
}

#[test]
fn options_retain_exact_numeric_types_debug_flags_and_structured_dependencies() -> TestResult {
    let owner = Owner::Template;
    let document = fixture::build(
        &fixture::database(options_properties(), owner),
        CatalogPublicationMode::Normal,
    )?;
    verify_metadata(&document, owner)?;
    verify_integer(&document, owner)?;
    verify_dependencies(&document, owner)?;
    verify_float_and_tags(&document, owner)?;
    Ok(())
}

fn options_properties() -> Vec<TestValue> {
    let integer = option("Integer")
        .named("integerOption")
        .value("value", TestValue::Int32(16_777_217))
        .value("variation", TestValue::Int32(i32::MIN));
    let dependent = TestInstance::new("DependentMissionProperty")
        .value("property", TestValue::pointer("integerProperty"))
        .value(
            "values",
            fixture::array(vec![
                TestValue::pointer("integerOption"),
                TestValue::Null,
                TestValue::pointer("integerOption"),
            ]),
        );
    let integer_property = fixture::object(
        TestInstance::new("MissionProperty")
            .named("integerProperty")
            .value("missionVariableName", TestValue::text("Integer"))
            .value(
                "value",
                fixture::object(
                    TestInstance::new("MissionPropertyValue_Integer")
                        .value("options", fixture::array(vec![fixture::object(integer)])),
                ),
            ),
    );
    let string = option("StringHash")
        .value("value", TestValue::text(""))
        .value(
            "dependentProperties",
            fixture::array(vec![fixture::object(dependent)]),
        );
    vec![
        integer_property,
        declaration("StringHash", vec![fixture::object(string)]),
        declaration(
            "Float",
            vec![fixture::object(
                option("Float")
                    .value("value", TestValue::Single(-0.125))
                    .value("variation", TestValue::Single(0.0625)),
            )],
        ),
        declaration(
            "Tags",
            vec![fixture::object(
                option("Tags")
                    .value("tags", fixture::object(fixture::tags()))
                    .value(
                        "negativeTags",
                        fixture::object(TestInstance::new("TagList")),
                    ),
            )],
        ),
    ]
}

fn verify_metadata(document: &Value, owner: Owner) -> TestResult {
    for kind in ["Integer", "StringHash", "Float", "Tags"] {
        let options = fixture::field(fixture::value(document, owner, kind)?, "/options")?
            .as_array()
            .ok_or("options missing")?;
        let option = options.first().ok_or("option missing")?;
        assert_eq!(
            *fixture::field(option, "/textId")?,
            "@option",
            "the authored source token is independent of display resolution"
        );
        assert_eq!(
            *fixture::field(option, "/weighting")?,
            0.375,
            "Single weighting must not be read as an integer"
        );
        assert_eq!(
            *fixture::field(option, "/debugForceChooseThisOption")?,
            false,
            "the explicit debug flag survives"
        );
        assert_eq!(
            *fixture::field(option, "/displayName/key")?,
            "@option",
            "the option label remains a localised text"
        );
    }
    Ok(())
}

fn verify_integer(document: &Value, owner: Owner) -> TestResult {
    let integer = fixture::value(document, owner, "Integer")?;
    assert_eq!(
        *fixture::field(integer, "/options/0/value")?,
        16_777_217,
        "Int32 must not lose precision through f32"
    );
    assert_eq!(
        *fixture::field(integer, "/options/0/variation")?,
        i32::MIN,
        "the signed bound survives exactly"
    );
    Ok(())
}

fn verify_dependencies(document: &Value, owner: Owner) -> TestResult {
    let string = fixture::value(document, owner, "StringHash")?;
    assert_eq!(
        *fixture::field(string, "/options/0/value")?,
        "",
        "an authored empty String is retained"
    );
    let dependency = fixture::field(string, "/options/0/dependentProperties/0")?;
    assert_eq!(
        *fixture::field(dependency, "/property/variable")?,
        "Integer",
        "dependentProperties is a Class structure with a property pointer"
    );
    assert_eq!(
        *fixture::field(dependency, "/values/0/targetType")?,
        "MissionPropertyValueOption_Integer",
        "option targets retain their own type"
    );
    assert_eq!(
        *fixture::field(dependency, "/values/0/status")?,
        "resolved",
        "an option weak target does not need a mission variable"
    );
    assert_eq!(
        *fixture::field(dependency, "/values/1")?,
        Value::Null,
        "null option pointers retain position"
    );
    assert_eq!(
        *fixture::field(dependency, "/values/0")?,
        *fixture::field(dependency, "/values/2")?,
        "repeated option targets retain position"
    );
    Ok(())
}

fn verify_float_and_tags(document: &Value, owner: Owner) -> TestResult {
    assert_eq!(
        *fixture::field(
            fixture::value(document, owner, "Float")?,
            "/options/0/value"
        )?,
        -0.125,
        "fractional values survive"
    );
    assert_eq!(
        *fixture::field(
            fixture::value(document, owner, "Float")?,
            "/options/0/variation"
        )?,
        0.0625,
        "fractional variation survives"
    );
    assert_eq!(
        *fixture::field(
            fixture::value(document, owner, "Tags")?,
            "/options/0/sourceTags/tags/1/status"
        )?,
        "dangling",
        "tag option raw target identity is retained"
    );
    Ok(())
}

#[test]
fn race_splits_and_faction_conditions_retain_single_values_and_target_identity() -> TestResult {
    let race = race();
    let conditions = vec![
        fixture::object(
            TestInstance::new("DataSetMatchCondition_FactionIdentityDef")
                .value("factionIdentity", TestValue::Reference(fixture::MISSING)),
        ),
        fixture::object(
            TestInstance::new("DataSetMatchCondition_ExcludeNearbyLocationsDef")
                .value("exclusionRangeKm", TestValue::Single(0.375)),
        ),
        fixture::object(
            TestInstance::new("DataSetMatchCondition_ExcludeDistantLocationsDef")
                .value("exclusionRangeKm", TestValue::Single(1.125)),
        ),
    ];
    let organization = TestInstance::new("MissionPropertyValue_Organization")
        .value("matchConditions", fixture::array(conditions));
    let owner = Owner::Generator;
    let document = fixture::build(
        &fixture::database(
            vec![
                fixture::property("Race", race),
                fixture::property("Faction", organization),
            ],
            owner,
        ),
        CatalogPublicationMode::Normal,
    )?;
    assert_eq!(
        *fixture::field(fixture::value(&document, owner, "Race")?, "/targetSplits")?,
        json!([{"targetSplit":0.125},{"targetSplit":0.0}]),
        "every authored split survives in order"
    );
    let conditions = fixture::field(fixture::value(&document, owner, "Faction")?, "/conditions")?;
    assert_eq!(
        *fixture::field(conditions, "/0/condition")?,
        "factionIdentity",
        "FactionIdentityDef is a supported union variant"
    );
    assert_eq!(
        *fixture::field(conditions, "/0/factionIdentity/status")?,
        "dangling",
        "the faction target is not discarded when dangling"
    );
    assert_eq!(
        *fixture::field(conditions, "/1/rangeKm")?,
        0.375,
        "nearby range has Single precision"
    );
    assert_eq!(
        *fixture::field(conditions, "/2/rangeKm")?,
        1.125,
        "distant range has Single precision"
    );
    Ok(())
}

#[test]
fn option_display_keys_keep_their_established_meaning_and_raw_text_ids_survive() -> TestResult {
    for owner in [Owner::Generator, Owner::Template, Owner::Broker] {
        let tokens = ["@LOC_UNINITIALIZED", "", "bare_label", "@option"];
        let properties = tokens
            .into_iter()
            .enumerate()
            .map(|(index, token)| {
                fixture::property(
                    &format!("Label_{index}"),
                    TestInstance::new("MissionPropertyValue_Integer").value(
                        "options",
                        fixture::array(vec![fixture::object(
                            option("Integer").value("textId", TestValue::text(token)),
                        )]),
                    ),
                )
            })
            .collect();
        let document = fixture::build(
            &fixture::database(properties, owner),
            CatalogPublicationMode::Normal,
        )?;
        for (index, token) in tokens.into_iter().enumerate() {
            let value = fixture::value(&document, owner, &format!("Label_{index}"))?;
            let option = fixture::field(value, "/options/0")?;
            assert_eq!(
                *fixture::field(option, "/textId")?,
                token,
                "the original Locale token survives unchanged"
            );
            let expected = if token == "@option" {
                json!(token)
            } else {
                Value::Null
            };
            assert_eq!(
                *fixture::field(option, "/displayName/key")?,
                expected,
                "existing GameText normalisation remains independent of raw source preservation"
            );
        }
    }
    Ok(())
}

fn race() -> TestInstance {
    TestInstance::new("MissionPropertyValue_TimeTrialRace").value(
        "targetSplits",
        fixture::array(vec![
            fixture::object(
                TestInstance::new("TimeTrialSplit").value("targetSplit", TestValue::Single(0.125)),
            ),
            fixture::object(
                TestInstance::new("TimeTrialSplit").value("targetSplit", TestValue::Single(0.0)),
            ),
        ]),
    )
}
