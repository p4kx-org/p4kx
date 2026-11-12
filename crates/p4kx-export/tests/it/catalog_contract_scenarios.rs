// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, DcbOrigin, TestResult, at, guid};

const GENERATOR: &str = "ContractGenerator";
const HANDLER: &str = "ContractGeneratorHandlerBase";
const CONCRETE_HANDLER: &str = "FixtureScenarioHandler";
const CONTRACT: &str = "ContractBase";
const CONCRETE_CONTRACT: &str = "FixtureScenarioContract";
const SCENARIO: &str = "MissionScenario";
const UNRELATED: &str = "FixtureUnrelatedOwner";
const FIELD: &str = "required_active_scenarios";
const GENERATOR_GUID: u128 = 0x0010_8680;
const SCENARIO_GUID: u128 = 0x0010_8681;
const MISSING_GUID: u128 = 0x0010_8682;
const CONTRACT_GUID: u128 = 0x0010_8683;

fn required_scenarios(owner: &str) -> TestStruct {
    TestStruct::new(owner).array(FIELD, TestType::ReferenceTo(SCENARIO.to_owned()))
}

fn scenario_schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(SCENARIO))
        .structure(required_scenarios(CONTRACT).field("id", TestType::Guid))
        .structure(TestStruct::new(CONCRETE_CONTRACT).parent(CONTRACT))
        .structure(required_scenarios(HANDLER))
        .structure(required_scenarios(UNRELATED))
        .structure(
            TestStruct::new(CONCRETE_HANDLER)
                .parent(HANDLER)
                .field(
                    "fixtureExtra",
                    TestType::StrongPointer(UNRELATED.to_owned()),
                )
                .array("contracts", TestType::StrongPointer(CONTRACT.to_owned())),
        )
        .structure(
            required_scenarios(GENERATOR)
                .array("generators", TestType::StrongPointer(HANDLER.to_owned())),
        )
}

fn database() -> DataForgeBuilder {
    let contract = TestInstance::new(CONCRETE_CONTRACT)
        .value("id", TestValue::Guid(CONTRACT_GUID))
        .value(
            FIELD,
            TestValue::Array(vec![
                TestValue::Reference(SCENARIO_GUID),
                TestValue::Reference(0),
                TestValue::Reference(MISSING_GUID),
                TestValue::Reference(SCENARIO_GUID),
            ]),
        );
    let unrelated = TestInstance::new(UNRELATED).value(FIELD, TestValue::Array(Vec::new()));
    let handler = TestInstance::new(CONCRETE_HANDLER)
        .value("fixtureExtra", TestValue::Struct(unrelated))
        .value(FIELD, TestValue::Array(Vec::new()))
        .value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(contract)]),
        );
    scenario_schema()
        .record(
            TestRecord::new(
                GENERATOR_GUID,
                GENERATOR,
                "ContractGenerator.Fixture",
                "libs/foundry/records/contracts/contractgenerator/fixture.xml",
            )
            .value(
                FIELD,
                TestValue::Array(vec![TestValue::Reference(SCENARIO_GUID)]),
            )
            .value(
                "generators",
                TestValue::Array(vec![TestValue::Struct(handler)]),
            ),
        )
        .record(TestRecord::new(
            SCENARIO_GUID,
            SCENARIO,
            "MissionScenario.Fixture",
            "libs/foundry/records/contracts/missionscenarios/fixture.xml",
        ))
}

#[test]
fn authored_scenario_requirements_preserve_each_physical_owner_and_inherited_slot() -> TestResult {
    let (_folder, archive) = fixture::open(&database(), DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &fixture::options()).map_err(|error| error.to_string())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let table = at(&catalog, "/declarations/contractScenarioRequirements")?;
    assert_eq!(at(table, "/availability")?, "available");
    let rows = at(table, "/rows")?
        .as_array()
        .ok_or("scenario requirement rows")?;
    assert_eq!(rows.len(), 3, "one declaration for each authored owner");
    for row in rows {
        assert_eq!(at(row, "/scenarios/containsEncryptedSource")?, true);
    }
    let root = at(table, "/rows/0/scenarios/value")?;
    assert_eq!(
        at(root, "/0/value/guid/value")?,
        &json!(guid(SCENARIO_GUID))
    );
    assert_eq!(at(table, "/rows/1/scenarios/value")?, &json!([]));
    let owned = at(table, "/rows/2/scenarios/value")?;
    assert_eq!(
        at(owned, "/0/value/guid/value")?,
        &json!(guid(SCENARIO_GUID))
    );
    assert_eq!(at(owned, "/1/state")?, "null");
    assert_eq!(at(owned, "/2/value/status")?, "dangling");
    assert_eq!(
        at(owned, "/3/value/guid/value")?,
        at(owned, "/0/value/guid/value")?
    );
    check_physical_occurrences(&catalog, rows)?;
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

fn check_physical_occurrences(catalog: &Value, rows: &[Value]) -> TestResult {
    let definition = at(catalog, "/definitions/contractGenerators/rows/0")?;
    let source_types = [None, Some(CONCRETE_HANDLER), Some(CONCRETE_CONTRACT)];
    for (row, expected_type) in rows.iter().zip(source_types) {
        let source = at(row, "/source")?;
        assert_eq!(
            at(source, "/recordIndex")?,
            at(definition, "/recordIndex/value")?,
            "the declaration points to its physical Generator Record"
        );
        assert_eq!(
            at(source, "/inputRef")?,
            at(definition, "/inputRef")?,
            "the declaration and Generator use the same DataForge input"
        );
        let array = at(source, "/scenariosValue/valueIndex")?
            .as_str()
            .ok_or("scenario declaration index")?;
        assert_eq!(
            at(
                definition,
                &format!("/source/values/{array}/value/value/type")
            )?,
            "array",
            "the structural reference resolves to the original Reference array"
        );
        match expected_type {
            None => assert!(
                at(source, "/owner")?.is_null(),
                "the Generator is the arena root"
            ),
            Some(expected) => {
                let owner = at(source, "/owner/valueIndex")?
                    .as_str()
                    .ok_or("owner index")?;
                assert_eq!(
                    at(
                        definition,
                        &format!("/source/values/{owner}/value/value/sourceType/value")
                    )?,
                    expected,
                    "the structural owner resolves to the actual concrete source type"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn authored_scenario_requirements_keep_filter_availability_and_clear_origin() -> TestResult {
    let (_folder, archive) = fixture::open(&database(), DcbOrigin::Plain)?;
    let built = build_catalog(&archive, &fixture::options()).map_err(|error| error.to_string())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        at(
            &catalog,
            "/declarations/contractScenarioRequirements/rows/0/scenarios/containsEncryptedSource"
        )?,
        false
    );
    let filter = Filter::new("*/missionscenarios/*");
    let mut options = fixture::options();
    options.filter = Some(&filter);
    let filtered = build_catalog(&archive, &options).map_err(|error| error.to_string())?;
    let catalog: Value = serde_json::from_slice(filtered.json())?;
    let table = at(&catalog, "/declarations/contractScenarioRequirements")?;
    assert_eq!(at(table, "/availability")?, "available");
    assert_eq!(at(table, "/rows")?, &json!([]));
    Ok(())
}
