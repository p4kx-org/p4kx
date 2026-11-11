// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mission_values_fixture::{self as fixture, Owner, TestResult};
use p4kx_export::CatalogPublicationMode;
use p4kx_testkit::{TestInstance, TestValue};
use serde_json::{Value, json};

use crate::loot_v3_graph as raw;
use crate::mission_canonical_text as canonical;
use crate::mission_template_graph::{self as source, Property};

#[path = "mission_values_option_targets.rs"]
mod targets;

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
    let builder = fixture::database(options_properties(), owner);
    crate::mission_template_original::owner_is_first(&builder)?;
    let document = fixture::build(&builder, CatalogPublicationMode::Normal)?;
    verify_metadata(&document)?;
    verify_integer(&document)?;
    verify_dependencies(&document)?;
    verify_float_and_tags(&document)?;
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

fn verify_metadata(document: &Value) -> TestResult {
    for (name, kind) in [
        ("Integer", "integer"),
        ("StringHash", "stringHash"),
        ("Float", "float"),
        ("Tags", "tags"),
    ] {
        let property = Property::new(document, name)?;
        property.kind(kind)?;
        let option = property.element(property.field(property.node, "options")?, 0)?;
        raw::structure(option, &format!("MissionPropertyValueOption_{name}"))?;
        let token = property.field(option, "textId")?;
        assert_eq!(
            source::text(token)?,
            "@option",
            "the original Locale token precedes presentation lookup"
        );
        canonical::single(property.field(option, "weighting")?, 0.375)?;
        assert_eq!(
            source::scalar(
                property.field(option, "DEBUG_forceChooseThisOption")?,
                "boolean"
            )?,
            &json!(false),
            "the explicit debug flag is preserved without a field alias"
        );
        canonical::key(
            &property,
            document,
            token,
            "@option",
            "lookup",
            Some("@option"),
        )?;
    }
    Ok(())
}

fn verify_integer(document: &Value) -> TestResult {
    let property = Property::new(document, "Integer")?;
    let option = property.element(property.field(property.node, "options")?, 0)?;
    for (field, expected) in [("value", 16_777_217), ("variation", i32::MIN)] {
        let number = source::scalar(property.field(option, field)?, "number")?;
        assert_eq!(
            number.get("type"),
            Some(&json!("int32")),
            "integer source width is retained exactly"
        );
        assert_eq!(
            number.get("value"),
            Some(&json!(expected)),
            "signed Int32 values never pass through f32"
        );
    }
    Ok(())
}

fn verify_dependencies(document: &Value) -> TestResult {
    let property = Property::new(document, "StringHash")?;
    let option = property.element(property.field(property.node, "options")?, 0)?;
    assert_eq!(
        source::text(property.field(option, "value")?)?,
        "",
        "an authored empty String survives"
    );
    let dependency = property.element(property.field(option, "dependentProperties")?, 0)?;
    raw::structure(dependency, "DependentMissionProperty")?;
    let target = property.field(dependency, "property")?;
    let binding = property.binding(target)?;
    assert_eq!(
        raw::present(
            binding
                .get("missionPropertyStatus")
                .ok_or("strict dependency status")?
        )?,
        &json!("resolved"),
        "the dependency points to a genuine same-owner MissionProperty"
    );
    let field = binding
        .pointer("/target/field")
        .ok_or("actual dependency variable source field")?;
    assert_eq!(
        source::text(property.bound(field)?)?,
        "Integer",
        "the dependency resolves the original variable"
    );
    let values = property.field(dependency, "values")?;
    assert_eq!(
        raw::elements(values)?.len(),
        3,
        "all original option-target slots survive"
    );
    verify_option_targets(&property, values)
}

fn verify_option_targets(property: &Property<'_>, values: &Value) -> TestResult {
    for index in [0, 2] {
        let node = property.element(values, index)?;
        let weak = source::scalar(node, "weakReference")?;
        assert_eq!(
            raw::present(
                weak.pointer("/target/sourceType")
                    .ok_or("actual option target type")?
            )?,
            &json!("MissionPropertyValueOption_Integer"),
            "a non-property option target keeps its own concrete type"
        );
        assert_eq!(
            raw::present(
                weak.get("targetRecordIndex")
                    .ok_or("actual same-owner option")?
            )?,
            &json!(0),
            "the option really resolves inside its independently checked physical Template owner"
        );
        assert!(
            raw::present(
                weak.pointer("/target/location")
                    .ok_or("actual owned option location")?
            )?
            .is_string(),
            "the resolved original option has its exact owner-local location"
        );
        assert_eq!(
            property.binding(node)?.get("missionPropertyStatus"),
            Some(&Value::Null),
            "a generic option target is not coerced into a MissionProperty"
        );
    }
    raw::source_null(property.element(values, 1)?);
    let first = source::scalar(property.element(values, 0)?, "weakReference")?;
    let repeated = source::scalar(property.element(values, 2)?, "weakReference")?;
    assert_eq!(
        first.pointer("/target/location/value"),
        repeated.pointer("/target/location/value"),
        "the repeated original pointer retains the same semantic target"
    );
    assert_ne!(
        property.element(values, 0)?.get("sourceOrdinal"),
        property.element(values, 2)?.get("sourceOrdinal"),
        "equal targets retain separate actual original occurrences"
    );
    Ok(())
}

fn verify_float_and_tags(document: &Value) -> TestResult {
    let property = Property::new(document, "Float")?;
    let option = property.element(property.field(property.node, "options")?, 0)?;
    canonical::single(property.field(option, "value")?, -0.125)?;
    canonical::single(property.field(option, "variation")?, 0.0625)?;
    let property = Property::new(document, "Tags")?;
    let option = property.element(property.field(property.node, "options")?, 0)?;
    let tags = property.field(property.field(option, "tags")?, "tags")?;
    assert_eq!(
        raw::elements(tags)?.len(),
        4,
        "tag source order remains independent of its lookup interpretation"
    );
    source::reference(
        property.element(tags, 1)?,
        "79000000-0000-4000-8000-000000000002",
        None,
    )?;
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
        if !matches!(owner, Owner::Generator) {
            canonical_display(&document, &tokens, owner)?;
            continue;
        }
        for (index, token) in tokens.into_iter().enumerate() {
            let value = fixture::generator_value(&document, &format!("Label_{index}"))?;
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

fn canonical_display(document: &Value, tokens: &[&str; 4], owner: Owner) -> TestResult {
    let meanings = [
        ("placeholder", None),
        ("empty", None),
        ("notKey", None),
        ("lookup", Some("@option")),
    ];
    for (index, (token, (interpretation, expected))) in tokens.iter().zip(meanings).enumerate() {
        let property = owner.property(document, &format!("Label_{index}"))?;
        property.kind("integer")?;
        let option = property.element(property.field(property.node, "options")?, 0)?;
        canonical::key(
            &property,
            document,
            property.field(option, "textId")?,
            token,
            interpretation,
            expected,
        )?;
    }
    Ok(())
}
