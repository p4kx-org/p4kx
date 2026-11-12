// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, DcbOrigin, TestResult, at, guid};

const SCENARIO: &str = "MissionScenario";
const VARIABLE: &str = "MissionVariableBase";
const FIXTURE_VARIABLE: &str = "FixtureScenarioVariable";
const CYCLE: &str = "MissionScenarioCycle";
const PHASE: &str = "MissionScenarioCyclePhase";
const SCHEDULE: &str = "MissionScenarioSchedule";
const CONSTRAINT: &str = "MissionScenarioScheduleConstraint";
const RECURRENCE: &str = "MissionScenarioScheduleRecurrence";
const SCENARIO_GUID: u128 = 0x0010_8690;
const PROGRESS_GUID: u128 = 0x0010_8691;
const TABLE: &str = "/definitions/missionScenarios";

fn scenario_schema() -> Vec<TestStruct> {
    vec![
        TestStruct::new(SCENARIO)
            .field("name", TestType::String)
            .field("description", TestType::String)
            .array("variables", TestType::StrongPointer(VARIABLE.to_owned()))
            .array("cycles", TestType::Class(CYCLE.to_owned()))
            .field("schedule", TestType::StrongPointer(SCHEDULE.to_owned()))
            .field("auto_create", TestType::Bool)
            .field("track_progress", TestType::Bool),
        TestStruct::new(VARIABLE),
        TestStruct::new(FIXTURE_VARIABLE)
            .parent(VARIABLE)
            .field("fixtureFlag", TestType::Bool),
        TestStruct::new(CYCLE)
            .field("name", TestType::String)
            .field("description", TestType::String)
            .array("phases", TestType::Class(PHASE.to_owned())),
        TestStruct::new(PHASE)
            .field("name", TestType::String)
            .field("description", TestType::String)
            .field("duration_seconds", TestType::UInt32),
        TestStruct::new(SCHEDULE)
            .field("enabled", TestType::Bool)
            .field(
                "start_constraint",
                TestType::StrongPointer(CONSTRAINT.to_owned()),
            )
            .field(
                "end_constraint",
                TestType::StrongPointer(CONSTRAINT.to_owned()),
            )
            .field("recurrence", TestType::StrongPointer(RECURRENCE.to_owned())),
        TestStruct::new(CONSTRAINT)
            .field("enabled", TestType::Bool)
            .field("epoch", TestType::Int32),
        TestStruct::new(RECURRENCE)
            .field("cron", TestType::String)
            .field("duration_seconds", TestType::UInt32),
        TestStruct::new("ScenarioProgress"),
    ]
}

fn database() -> DataForgeBuilder {
    let phase = TestInstance::new(PHASE)
        .value("name", TestValue::text("Fixture phase"))
        .value("description", TestValue::text("Fixture phase description"))
        .value("duration_seconds", TestValue::UInt32(0));
    let cycle = TestInstance::new(CYCLE)
        .value("name", TestValue::text("Fixture cycle"))
        .value("description", TestValue::text("Fixture cycle description"))
        .value("phases", TestValue::Array(vec![TestValue::Struct(phase)]));
    scenario_schema()
        .into_iter()
        .fold(
            DataForgeBuilder::new().version(8),
            DataForgeBuilder::structure,
        )
        .record(
            TestRecord::new(
                SCENARIO_GUID,
                SCENARIO,
                "MissionScenario.Fixture",
                "libs/foundry/records/fixture_scenario_extension/fixture.xml",
            )
            .value("name", TestValue::text("Fixture scenario"))
            .value(
                "description",
                TestValue::text("Fixture protected description"),
            )
            .value(
                "variables",
                TestValue::Array(vec![
                    TestValue::Struct(
                        TestInstance::new(FIXTURE_VARIABLE)
                            .value("fixtureFlag", TestValue::Bool(true)),
                    ),
                    TestValue::Null,
                ]),
            )
            .value("cycles", TestValue::Array(vec![TestValue::Struct(cycle)]))
            .value("schedule", TestValue::Struct(scenario_schedule()))
            .value("auto_create", TestValue::Bool(false))
            .value("track_progress", TestValue::Bool(true)),
        )
        .record(TestRecord::new(
            PROGRESS_GUID,
            "ScenarioProgress",
            "ScenarioProgress.Unrelated",
            "libs/foundry/records/missionscenarios/unrelated.xml",
        ))
}

fn scenario_schedule() -> TestInstance {
    let start = TestInstance::new(CONSTRAINT)
        .value("enabled", TestValue::Bool(true))
        .value("epoch", TestValue::Int32(-1));
    let recurrence = TestInstance::new(RECURRENCE)
        .value("cron", TestValue::text("0 * * * *"))
        .value("duration_seconds", TestValue::UInt32(60));
    TestInstance::new(SCHEDULE)
        .value("enabled", TestValue::Bool(false))
        .value("start_constraint", TestValue::Struct(start))
        .value("end_constraint", TestValue::Null)
        .value("recurrence", TestValue::Struct(recurrence))
}

fn source_field<'row>(arena: &'row Value, fields: &Value, name: &str) -> TestResult<&'row Value> {
    let members = fields.as_array().ok_or("original fields array")?;
    let field = members
        .iter()
        .find(|member| member.pointer("/name/value").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| format!("missing original field {name}"))?;
    source_value(arena, at(field, "/value")?)
}

fn source_value<'row>(arena: &'row Value, reference: &Value) -> TestResult<&'row Value> {
    let index = at(reference, "/valueIndex")?
        .as_str()
        .ok_or("original value index")?;
    at(arena, &format!("/values/{index}/value"))
}

#[test]
fn mission_scenario_definitions_keep_source_schedule_cycles_and_identity() -> TestResult {
    let catalog = export(DcbOrigin::Encrypted, None)?;
    let table = at(&catalog, TABLE)?;
    assert_eq!(at(table, "/availability")?, "available");
    let [row] = at(table, "/rows")?
        .as_array()
        .ok_or("scenario rows")?
        .as_slice()
    else {
        return Err("one physical MissionScenario definition".into());
    };
    assert_eq!(at(row, "/guid/value")?, &json!(guid(SCENARIO_GUID)));
    assert_eq!(at(row, "/sourceType/value")?, SCENARIO);
    assert_eq!(at(row, "/guid/containsEncryptedSource")?, true);

    let arena = at(row, "/authoredFields")?;
    let roots = at(arena, "/roots")?;
    let description = source_field(arena, roots, "description")?;
    assert_eq!(at(description, "/value/value/type")?, "gameText");
    assert_eq!(at(description, "/containsEncryptedSource")?, true);
    let variables = source_field(arena, roots, "variables")?;
    assert_eq!(
        at(
            source_value(arena, at(variables, "/value/elements/0")?)?,
            "/value/sourceType/value"
        )?,
        FIXTURE_VARIABLE
    );
    assert_eq!(
        at(
            source_value(arena, at(variables, "/value/elements/1")?)?,
            "/state"
        )?,
        "null"
    );
    let cycles = source_field(arena, roots, "cycles")?;
    let cycle = source_value(arena, at(cycles, "/value/elements/0")?)?;
    let phases = source_field(arena, at(cycle, "/value/fields")?, "phases")?;
    let phase = source_value(arena, at(phases, "/value/elements/0")?)?;
    let seconds = source_field(arena, at(phase, "/value/fields")?, "duration_seconds")?;
    assert_eq!(
        at(seconds, "/value/value")?,
        &json!({"type":"uint32","value":0})
    );
    let schedule = source_field(arena, roots, "schedule")?;
    let fields = at(schedule, "/value/fields")?;
    assert_eq!(
        at(source_field(arena, fields, "enabled")?, "/value/value")?,
        false
    );
    assert_eq!(
        at(source_field(arena, fields, "end_constraint")?, "/state")?,
        "null"
    );
    let start = source_field(arena, fields, "start_constraint")?;
    let epoch = source_field(arena, at(start, "/value/fields")?, "epoch")?;
    assert_eq!(
        at(epoch, "/value/value")?,
        &json!({"type":"int32","value":-1})
    );
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

#[test]
fn mission_scenario_definitions_keep_availability_with_filtered_owners() -> TestResult {
    let filter = Filter::new("*/unrelated.xml");
    let catalog = export(DcbOrigin::Plain, Some(&filter))?;
    let table = at(&catalog, TABLE)?;
    assert_eq!(at(table, "/availability")?, "available");
    assert_eq!(at(table, "/rows")?, &json!([]));
    Ok(())
}

fn export(origin: DcbOrigin, filter: Option<&Filter>) -> TestResult<Value> {
    let (_folder, archive) = fixture::open(&database(), origin)?;
    let mut options = fixture::options();
    options.filter = filter;
    let built = build_catalog(&archive, &options).map_err(|error| error.to_string())?;
    Ok(serde_json::from_slice(built.json())?)
}
