// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, DcbOrigin, TestResult, at, guid};

const GENERATOR: &str = "ContractGenerator";
const HANDLER: &str = "ContractGeneratorHandlerBase";
const PARAMS: &str = "LocalContractGeneratorParams";
const AVAILABILITY: &str = "ContractAvailability";
const DNF: &str = "TagsDNF";
const TERM: &str = "TagsDNFTerm";
const SCOPE: &str = "ContractScope";
const BOARD: &str = "MissionBoardAvailability";
const TAG: &str = "Tag";
const IDENTITY: &str = "FactionIdentity";
const TAG_GUID: u128 = 0x0010_8692;
const IDENTITY_GUID: u128 = 0x0010_8693;
const GENERATOR_GUID: u128 = 0x0010_8694;
const MISSING_GUID: u128 = 0x0010_8695;

fn schema() -> DataForgeBuilder {
    let scope_type = TestType::Enum(SCOPE.to_owned());
    let board_type = TestType::Enum(BOARD.to_owned());
    let tag_reference = TestType::ReferenceTo(TAG.to_owned());
    DataForgeBuilder::new()
        .version(8)
        .enumeration(SCOPE, ["FixturePlanet", "FixtureCluster"])
        .enumeration(BOARD, ["FixtureBoardOnly", "FixtureEverywhere"])
        .structure(TestStruct::new(TAG))
        .structure(TestStruct::new(IDENTITY))
        .structure(
            TestStruct::new(TERM)
                .array("positiveTags", tag_reference.clone())
                .array("negativeTags", tag_reference),
        )
        .structure(TestStruct::new(DNF).array("terms", TestType::Class(TERM.to_owned())))
        .structure(
            TestStruct::new(PARAMS)
                .field("scope", scope_type)
                .field("parentLocationGeneralTags", TestType::Class(DNF.to_owned()))
                .array(
                    "factionIdentities",
                    TestType::ReferenceTo(IDENTITY.to_owned()),
                ),
        )
        .structure(TestStruct::new(AVAILABILITY).field("missionBoardAvailability", board_type))
        .structure(
            TestStruct::new(HANDLER)
                .field(
                    "localGeneratorParams",
                    TestType::StrongPointer(PARAMS.to_owned()),
                )
                .field(
                    "defaultAvailability",
                    TestType::Class(AVAILABILITY.to_owned()),
                ),
        )
        .structure(
            TestStruct::new(GENERATOR)
                .array("generators", TestType::StrongPointer(HANDLER.to_owned())),
        )
}

fn parameters() -> TestInstance {
    let term = TestInstance::new(TERM)
        .value(
            "positiveTags",
            TestValue::Array(vec![
                TestValue::Reference(TAG_GUID),
                TestValue::Reference(TAG_GUID),
            ]),
        )
        .value(
            "negativeTags",
            TestValue::Array(vec![TestValue::Reference(MISSING_GUID), TestValue::Null]),
        );
    let empty_term = TestInstance::new(TERM)
        .value("positiveTags", TestValue::Array(Vec::new()))
        .value("negativeTags", TestValue::Array(Vec::new()));
    let dnf = TestInstance::new(DNF).value(
        "terms",
        TestValue::Array(vec![TestValue::Struct(term), TestValue::Struct(empty_term)]),
    );
    TestInstance::new(PARAMS)
        .value("scope", TestValue::text("FixturePlanet"))
        .value("parentLocationGeneralTags", TestValue::Struct(dnf))
        .value(
            "factionIdentities",
            TestValue::Array(vec![
                TestValue::Reference(IDENTITY_GUID),
                TestValue::Reference(MISSING_GUID),
                TestValue::Null,
            ]),
        )
}

fn handler(parameters: TestValue, board: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new(HANDLER)
            .value("localGeneratorParams", parameters)
            .value(
                "defaultAvailability",
                TestValue::Struct(
                    TestInstance::new(AVAILABILITY)
                        .value("missionBoardAvailability", TestValue::text(board)),
                ),
            ),
    )
}

fn database() -> DataForgeBuilder {
    schema()
        .record(TestRecord::new(
            TAG_GUID,
            TAG,
            "Tag.Fixture",
            "libs/foundry/records/tags/fixture.xml",
        ))
        .record(TestRecord::new(
            IDENTITY_GUID,
            IDENTITY,
            "FactionIdentity.Fixture",
            "libs/foundry/records/factions/fixture_identity.xml",
        ))
        .record(
            TestRecord::new(
                GENERATOR_GUID,
                GENERATOR,
                "ContractGenerator.FixtureScopes",
                "libs/foundry/records/contracts/contractgenerator/fixture_scopes.xml",
            )
            .value(
                "generators",
                TestValue::Array(vec![
                    handler(TestValue::Struct(parameters()), "FixtureBoardOnly"),
                    handler(TestValue::Null, "FixtureEverywhere"),
                ]),
            ),
        )
}

#[test]
fn local_contract_generators_preserve_ordered_terms_and_actual_handler_scope() -> TestResult {
    let (_folder, archive) = fixture::open(&database(), DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &fixture::options()).map_err(|error| error.to_string())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let table = at(&catalog, "/configurations/localContractGenerators")?;
    assert_eq!(at(table, "/availability")?, "available");
    let rows = at(table, "/rows")?.as_array().ok_or("local scope rows")?;
    assert_eq!(
        rows.len(),
        2,
        "each authored Handler has independent configuration"
    );
    let local = at(table, "/rows/0/localGeneratorParams/value")?;
    assert_eq!(at(local, "/scope/value")?, "FixturePlanet");
    assert_eq!(at(local, "/scope/containsEncryptedSource")?, true);
    let terms = at(local, "/parentLocationGeneralTags/value/terms/value")?;
    let first = at(terms, "/0/value")?;
    assert_eq!(
        at(first, "/positiveTags/value/0/value/guid/value")?,
        &json!(guid(TAG_GUID))
    );
    assert_eq!(
        at(first, "/positiveTags/value/1/value/guid/value")?,
        &json!(guid(TAG_GUID))
    );
    assert_eq!(at(first, "/negativeTags/value/0/value/status")?, "dangling");
    assert_eq!(at(first, "/negativeTags/value/1/state")?, "null");
    assert_eq!(at(terms, "/1/value/positiveTags/value")?, &json!([]));
    assert_eq!(
        at(local, "/factionIdentities/value/0/value/guid/value")?,
        &json!(guid(IDENTITY_GUID))
    );
    assert_eq!(
        at(local, "/factionIdentities/value/1/value/status")?,
        "dangling"
    );
    assert_eq!(at(local, "/factionIdentities/value/2/state")?, "null");
    assert_eq!(at(table, "/rows/1/localGeneratorParams/state")?, "null");
    assert_eq!(
        at(
            table,
            "/rows/0/defaultAvailability/value/missionBoardAvailability/value"
        )?,
        "FixtureBoardOnly"
    );
    assert_eq!(
        at(
            table,
            "/rows/1/defaultAvailability/value/missionBoardAvailability/value"
        )?,
        "FixtureEverywhere"
    );
    assert_handler_sources(&catalog, rows)?;
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

fn assert_handler_sources(catalog: &Value, rows: &[Value]) -> TestResult {
    let definition = at(catalog, "/definitions/contractGenerators/rows/0")?;
    let mut handler_indices = Vec::new();
    for row in rows {
        let source = at(row, "/source")?;
        assert_eq!(
            at(source, "/inputRef")?,
            at(definition, "/inputRef")?,
            "the configuration and its Generator use the same DataForge input"
        );
        assert_eq!(
            at(source, "/recordIndex")?,
            at(definition, "/recordIndex/value")?,
            "the configuration points to the exact physical Generator Record"
        );
        let handler = at(source, "/handler/valueIndex")?
            .as_str()
            .ok_or("handler index")?;
        assert_eq!(
            at(
                definition,
                &format!("/source/values/{handler}/value/value/sourceType/value")
            )?,
            HANDLER,
            "each configuration references an original Handler occurrence"
        );
        handler_indices.push(handler.parse::<u64>()?);
    }
    let [first, second] = handler_indices.as_slice() else {
        return Err("expected two original Handler occurrences".into());
    };
    assert!(first < second, "physical Handler order");
    Ok(())
}

#[test]
fn current_handler_shape_reports_scope_configuration_as_absent() -> TestResult {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(AVAILABILITY))
        .structure(TestStruct::new(HANDLER).field(
            "defaultAvailability",
            TestType::Class(AVAILABILITY.to_owned()),
        ))
        .structure(
            TestStruct::new(GENERATOR)
                .array("generators", TestType::StrongPointer(HANDLER.to_owned())),
        )
        .record(
            TestRecord::new(
                GENERATOR_GUID,
                GENERATOR,
                "ContractGenerator.FixtureCurrent",
                "libs/foundry/records/contracts/contractgenerator/fixture_current.xml",
            )
            .value(
                "generators",
                TestValue::Array(vec![TestValue::Struct(TestInstance::new(HANDLER).value(
                    "defaultAvailability",
                    TestValue::Struct(TestInstance::new(AVAILABILITY)),
                ))]),
            ),
        );
    let (_folder, archive) = fixture::open(&database, DcbOrigin::Plain)?;
    let built = build_catalog(&archive, &fixture::options()).map_err(|error| error.to_string())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let table = at(&catalog, "/configurations/localContractGenerators")?;
    assert_eq!(at(table, "/availability")?, "absent");
    assert_eq!(at(table, "/rows")?, &json!([]));
    assert_eq!(
        at(
            &catalog,
            "/definitions/contractGenerators/rows/0/guid/value"
        )?,
        &json!(guid(GENERATOR_GUID))
    );
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}
