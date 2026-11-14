// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{PreparedDefinition, SOURCE_TYPE};
use crate::build::original_input::test_support::{self, TestResult};
use crate::record_observation::RecordObservation;

const MODULE: u128 = 1;
const MISSING: u128 = 2;
const TYPE_BASE: &str = "ModuleDeclarationType_Base";
const MISSION: &str = "ModuleDeclarationType_Mission";
const ENTITY_BASE: &str = "ModuleLocationEntities";
const STATIC: &str = "ModuleLocationEntities_Static";
const STRATEGY: &str = "ModuleLocationEntitiesAcquiringStrategy_Base";
const ALL: &str = "ModuleLocationEntitiesAcquiringStrategy_AllSuitable";
const SPAWN_BASE: &str = "ModuleSpawnedEntities";
const NPC: &str = "ModuleSpawnedEntities_NPC";
const UNKNOWN: &str = "FixtureModuleSpawnedEntities";
const VARIABLE_BASE: &str = "VariableDeclaration_Base";
const LOCATION_VARIABLE: &str = "VariableDeclaration_LocationEntities";

#[test]
fn module_declaration_preserves_authored_slots_and_link_identity() -> TestResult {
    let fixture = fixture().build();
    test_support::with_projection(&fixture, |forge, projection| {
        let record = forge
            .record(0)
            .ok_or("the original ModuleDeclaration Record")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(projection.original(), &record)?;
        let (prepared, notes) = super::read(&tree, projection, &admission, None)?;
        let table = PreparedDefinition::table(forge, SOURCE_TYPE, vec![prepared]);
        let row = serde_json::to_value(table.rows.first().ok_or("projected module")?)?;
        assert_eq!(
            notes.len(),
            1,
            "only the unknown spawn subtype needs a note"
        );
        check_links(&row)?;
        check_slots(&row)?;
        assert_value(
            &row,
            "/properties/value/0/value/value/state",
            &json!("null"),
        )
    })
}

fn check_links(row: &Value) -> TestResult {
    let module_type = at(row, "/moduleType/value")?;
    assert_value(module_type, "/kind", &json!("mission"))?;
    assert_sourced(module_type, "/pausesLocationNpcAutospawns", &json!(false))?;
    let links = at(module_type, "/requiredLocationModules/value")?;
    assert_value(links, "/0/value/targetRecordIndex/value", &json!(0))?;
    assert_value(links, "/1/value/status", &json!("dangling"))?;
    assert_value(links, "/2/state", &json!("null"))?;
    assert_value(links, "/3/value/targetRecordIndex/value", &json!(0))?;
    assert_value(module_type, "/requiredResourceSlot/state", &json!("null"))?;
    let location = at(row, "/locationEntities/value/0/value")?;
    assert_value(location, "/kind", &json!("static"))?;
    assert_value(
        location,
        "/acquiringStrategy/value/kind",
        &json!("allSuitable"),
    )?;
    assert_sourced(location, "/acquiringStrategy/value/minCount", &json!(0))?;
    assert_sourced(location, "/acquiringStrategy/value/maxCount", &json!(5))?;
    assert_value(
        location,
        "/entityDeclarations/value/0/value/status",
        &json!("dangling"),
    )?;
    let variable = at(row, "/declaredVariables/value/0/value")?;
    assert_value(variable, "/kind", &json!("locationEntities"))?;
    assert_value(
        variable,
        "/locationEntities/value/0/value/target/type",
        &json!("instance"),
    )?;
    assert_value(
        variable,
        "/locationEntities/value/0/value/target/location/value",
        &json!("locationEntities[0]"),
    )?;
    assert_value(variable, "/locationEntities/value/1/state", &json!("null"))
}

fn check_slots(row: &Value) -> TestResult {
    let spawn = at(row, "/spawnedEntities/value")?;
    let npc = at(spawn, "/0/value")?;
    assert_value(npc, "/kind", &json!("npc"))?;
    assert_sourced(npc, "/debugName", &json!("Fixture NPC"))?;
    assert_sourced(npc, "/spawnImmediately", &json!(false))?;
    assert_sourced(
        npc,
        "/spawnDescriptionPropertyName",
        &json!("FixtureSpawnProperty"),
    )?;
    assert_value(spawn, "/1/state", &json!("null"))?;
    let unknown = at(spawn, "/2/value")?;
    assert_value(unknown, "/kind", &json!("authored"))?;
    assert_sourced(unknown, "/sourceType", &json!(UNKNOWN))?;
    assert!(
        at(unknown, "/authoredFields/roots")?
            .as_array()
            .is_some_and(|fields| !fields.is_empty()),
        "future spawn fields retain original shape"
    );
    Ok(())
}

fn assert_sourced(value: &Value, path: &str, expected: &Value) -> TestResult {
    let cell = at(value, path)?;
    assert_value(cell, "/state", &json!("present"))?;
    assert_value(cell, "/value", expected)?;
    assert_value(cell, "/containsEncryptedSource", &json!(false))?;
    assert_value(cell, "/sourceSetComplete", &json!(true))?;
    assert!(
        at(cell, "/sourceUsesRefs")?
            .as_array()
            .is_some_and(|uses| !uses.is_empty()),
        "original sources for {path}"
    );
    Ok(())
}

fn assert_value(value: &Value, path: &str, expected: &Value) -> TestResult {
    assert_eq!(at(value, path)?, expected, "original value at {path}");
    Ok(())
}

fn at<'value>(value: &'value Value, path: &str) -> TestResult<&'value Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing {path}").into())
}

fn fixture() -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("LocationResourceSlot"))
        .structure(TestStruct::new("LocationEntityDeclaration"))
        .structure(TestStruct::new(TYPE_BASE))
        .structure(
            TestStruct::new(MISSION)
                .parent(TYPE_BASE)
                .array("requiredLocationModules", reference(SOURCE_TYPE))
                .field("requiredResourceSlot", reference("LocationResourceSlot"))
                .field("pausesLocationNPCAutospawns", TestType::Bool),
        )
        .structure(TestStruct::new(STRATEGY))
        .structure(
            TestStruct::new(ALL)
                .parent(STRATEGY)
                .field("minCount", TestType::Int32)
                .field("maxCount", TestType::Int32),
        )
        .structure(
            TestStruct::new(ENTITY_BASE)
                .field("debugName", TestType::String)
                .field("requiredForInit", TestType::Bool)
                .field("optional", TestType::Bool)
                .field("acquiringStrategy", strong(STRATEGY)),
        )
        .structure(
            TestStruct::new(STATIC)
                .parent(ENTITY_BASE)
                .array("entityDeclarations", reference("LocationEntityDeclaration")),
        );
    content_types(builder).record(module())
}

fn content_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new(SPAWN_BASE)
                .field("debugName", TestType::String)
                .field("spawnImmediately", TestType::Bool),
        )
        .structure(
            TestStruct::new(NPC)
                .parent(SPAWN_BASE)
                .field("spawnDescriptionPropertyName", TestType::String),
        )
        .structure(
            TestStruct::new(UNKNOWN)
                .parent(SPAWN_BASE)
                .field("fixtureFuture", TestType::Int32),
        )
        .structure(
            TestStruct::new(VARIABLE_BASE).field("subsumptionVariableName", TestType::String),
        )
        .structure(
            TestStruct::new(LOCATION_VARIABLE)
                .parent(VARIABLE_BASE)
                .array(
                    "locationEntities",
                    TestType::WeakPointer(ENTITY_BASE.to_owned()),
                ),
        )
        .structure(TestStruct::new("BaseMissionPropertyValue"))
        .structure(
            TestStruct::new("MissionProperty")
                .field("missionVariableName", TestType::String)
                .field("extendedTextToken", TestType::String)
                .field("value", strong("BaseMissionPropertyValue")),
        )
        .structure(
            TestStruct::new(SOURCE_TYPE)
                .field("module", TestType::String)
                .field("moduleType", strong(TYPE_BASE))
                .array("locationEntities", strong(ENTITY_BASE))
                .array("properties", TestType::Class("MissionProperty".to_owned()))
                .array("spawnedEntities", strong(SPAWN_BASE))
                .array("declaredVariables", strong(VARIABLE_BASE)),
        )
}

fn module() -> TestRecord {
    TestRecord::new(
        MODULE,
        SOURCE_TYPE,
        "ModuleDeclaration.Fixture",
        "fixture/module.xml",
    )
    .value("module", TestValue::text("Fixture"))
    .value(
        "moduleType",
        TestValue::Struct(
            TestInstance::new(MISSION)
                .value(
                    "requiredLocationModules",
                    TestValue::Array(vec![
                        TestValue::Reference(MODULE),
                        TestValue::Reference(MISSING),
                        TestValue::Null,
                        TestValue::Reference(MODULE),
                    ]),
                )
                .value("pausesLocationNPCAutospawns", TestValue::Bool(false)),
        ),
    )
    .value(
        "locationEntities",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new(STATIC)
                .named("fixture-location")
                .value(
                    "acquiringStrategy",
                    TestValue::Struct(
                        TestInstance::new(ALL)
                            .value("minCount", TestValue::Int32(0))
                            .value("maxCount", TestValue::Int32(5)),
                    ),
                )
                .value(
                    "entityDeclarations",
                    TestValue::Array(vec![TestValue::Reference(MISSING)]),
                ),
        )]),
    )
    .value("properties", properties())
    .value("spawnedEntities", spawns())
    .value(
        "declaredVariables",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new(LOCATION_VARIABLE).value(
                "locationEntities",
                TestValue::Array(vec![
                    TestValue::pointer("fixture-location"),
                    TestValue::Null,
                ]),
            ),
        )]),
    )
}

fn properties() -> TestValue {
    TestValue::Array(vec![TestValue::Struct(
        TestInstance::new("MissionProperty").value(
            "missionVariableName",
            TestValue::text("FixtureSpawnProperty"),
        ),
    )])
}

fn spawns() -> TestValue {
    TestValue::Array(vec![
        TestValue::Struct(
            TestInstance::new(NPC)
                .value("debugName", TestValue::text("Fixture NPC"))
                .value("spawnImmediately", TestValue::Bool(false))
                .value(
                    "spawnDescriptionPropertyName",
                    TestValue::text("FixtureSpawnProperty"),
                ),
        ),
        TestValue::Null,
        TestValue::Struct(TestInstance::new(UNKNOWN).value("fixtureFuture", TestValue::Int32(-3))),
    ])
}

fn strong(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}
