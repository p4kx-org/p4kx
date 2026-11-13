// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at};

const ROOT: &str = "MissionLocationTemplate";
const LOCATION: &str = "MissionLocationData";
const MODULE: &str = "SMissionLocationModule";
const INPUT: &str = "AbstractMissionInitParam";
const PROPERTY: &str = "MissionProperty";
const PROPERTY_VALUE: &str = "BaseMissionPropertyValue";

#[test]
fn module_keeps_typed_inputs_and_original_property_slots() -> TestResult {
    let (_directory, archive) = common::open(&database(), DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let module = at(
        &catalog,
        "/missionLocationTemplates/0/configuration/value/missionModules/value/0/value",
    )?;
    assert_eq!(at(module, "/isPersistent/value")?, false, "explicit false");
    for (path, expected) in [
        ("/missionInputs/value/0/value/kind", json!("boolean")),
        ("/missionInputs/value/0/value/value/value", json!(false)),
        ("/missionInputs/value/1/value/kind", json!("integer")),
        ("/missionInputs/value/1/value/value/value", json!(-1)),
        ("/missionInputs/value/2/state", json!("null")),
        (
            "/missionInputs/value/3/value/value/value/rawBits",
            json!("7fc04321"),
        ),
        (
            "/missionInputs/value/3/value/value/containsEncryptedSource",
            json!(true),
        ),
        ("/missionInputs/value/4/value/kind", json!("activity")),
        (
            "/missionInputs/value/5/value/value/value/negativeTags/value",
            json!([]),
        ),
        ("/missionInputs/value/6/value/kind", json!("authored")),
        (
            "/properties/value/0/value/missionVariableName/value",
            json!("FixtureProperty"),
        ),
        (
            "/properties/value/0/value/value/value/sourceType/value",
            json!("MissionPropertyValue_Boolean"),
        ),
        (
            "/properties/value/0/value/value/value/authoredFields/values/0/value/value/value",
            json!(true),
        ),
        ("/properties/value/1/value/value/state", json!("null")),
    ] {
        assert_eq!(
            at(module, path)?,
            &expected,
            "original module value at {path}"
        );
    }
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(&catalog) {
        return Err(format!("{}: {}", error.instance_path(), error).into());
    }
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

fn database() -> DataForgeBuilder {
    input_schema(DataForgeBuilder::new().version(8))
        .structure(TestStruct::new(PROPERTY_VALUE))
        .structure(
            TestStruct::new("MissionPropertyValue_Boolean")
                .parent(PROPERTY_VALUE)
                .field("value", TestType::Bool),
        )
        .structure(
            TestStruct::new(PROPERTY)
                .field("missionVariableName", TestType::String)
                .field("extendedTextToken", TestType::String)
                .field("value", TestType::StrongPointer(PROPERTY_VALUE.into())),
        )
        .structure(
            TestStruct::new(MODULE)
                .field("isPersistent", TestType::Bool)
                .field("missionModule", TestType::String)
                .array("missionInputs", TestType::StrongPointer(INPUT.into()))
                .array("properties", TestType::Class(PROPERTY.into())),
        )
        .structure(
            TestStruct::new(LOCATION).array("missionModules", TestType::Class(MODULE.into())),
        )
        .structure(TestStruct::new(ROOT).field("locationData", TestType::Class(LOCATION.into())))
        .record(
            TestRecord::new(
                0x1074_0301,
                ROOT,
                "MissionLocationTemplate.FixtureModule",
                "libs/foundry/records/missiondata/pu_locations/fixture/module.xml",
            )
            .value(
                "locationData",
                node(
                    TestInstance::new(LOCATION)
                        .value("missionModules", TestValue::Array(vec![module()])),
                ),
            ),
        )
}

fn input_schema(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(TestStruct::new("Tag"))
        .structure(
            TestStruct::new("TagSearchTerm")
                .array("positiveTags", TestType::ReferenceTo("Tag".into()))
                .array("negativeTags", TestType::ReferenceTo("Tag".into())),
        )
        .structure(TestStruct::new(INPUT).field("name", TestType::String))
        .structure(input_type("MissionInitParamBoolean", TestType::Bool))
        .structure(input_type("MissionInitParamInteger", TestType::Int32))
        .structure(input_type("MissionInitParamFloat", TestType::Single))
        .structure(input_type("MissionInitParamActivity", TestType::String))
        .structure(input_type(
            "MissionInitParamTag",
            TestType::Class("TagSearchTerm".into()),
        ))
        .structure(input_type("FixtureFutureModuleInput", TestType::Int32))
}

fn module() -> TestValue {
    node(
        TestInstance::new(MODULE)
            .value("isPersistent", TestValue::Bool(false))
            .value("missionModule", TestValue::text("FixtureModule"))
            .value(
                "missionInputs",
                TestValue::Array(vec![
                    parameter("MissionInitParamBoolean", TestValue::Bool(false)),
                    parameter("MissionInitParamInteger", TestValue::Int32(-1)),
                    TestValue::Null,
                    parameter(
                        "MissionInitParamFloat",
                        TestValue::Single(f32::from_bits(0x7fc0_4321)),
                    ),
                    parameter(
                        "MissionInitParamActivity",
                        TestValue::text("FixtureActivity"),
                    ),
                    parameter(
                        "MissionInitParamTag",
                        node(
                            TestInstance::new("TagSearchTerm")
                                .value("positiveTags", TestValue::Array(vec![TestValue::Null]))
                                .value("negativeTags", TestValue::Array(Vec::new())),
                        ),
                    ),
                    parameter("FixtureFutureModuleInput", TestValue::Int32(4)),
                ]),
            )
            .value(
                "properties",
                TestValue::Array(vec![
                    property(node(
                        TestInstance::new("MissionPropertyValue_Boolean")
                            .value("value", TestValue::Bool(true)),
                    )),
                    property(TestValue::Null),
                ]),
            ),
    )
}

fn property(value: TestValue) -> TestValue {
    node(
        TestInstance::new(PROPERTY)
            .value("missionVariableName", TestValue::text("FixtureProperty"))
            .value("extendedTextToken", TestValue::text(""))
            .value("value", value),
    )
}

fn input_type(name: &str, storage: TestType) -> TestStruct {
    TestStruct::new(name).parent(INPUT).field("value", storage)
}

fn parameter(name: &str, value: TestValue) -> TestValue {
    node(
        TestInstance::new(name)
            .value("name", TestValue::text("FixtureInput"))
            .value("value", value),
    )
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
