// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::TestResult;

pub(super) const PRIMARY_INDEX: u32 = 0;
const PRIMARY: u128 = 1;
const OTHER: u128 = 2;
const SCENARIO: u128 = 3;
const GENERATOR: &str = "ContractGenerator";
const HANDLER: &str = "ContractGeneratorHandlerBase";
const PARAMS: &str = "LocalContractGeneratorParams";
const AVAILABILITY: &str = "ContractAvailability";
const SCENARIO_TYPE: &str = "MissionScenario";
const SCOPE: &str = "ContractScope";
const BOARD: &str = "MissionBoardAvailability";
const REQUIRED: &str = "required_active_scenarios";

#[derive(Clone, Copy)]
pub(super) enum Input {
    Encrypted,
    LockedLocale,
}

pub(super) fn open(input: Input) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("synthetic-generator-facets.p4k");
    let dataforge = TestEntry::stored("Data/Game2.dcb", database());
    let locales = TestEntry::stored(
        Language::English.global_ini_path(),
        b"fixture=Locked translation\n",
    );
    let (dataforge, locales, key) = match input {
        Input::Encrypted => (
            dataforge.encrypted(),
            locales,
            Some(p4kx_testkit::test_key()),
        ),
        Input::LockedLocale => (dataforge, locales.encrypted(), None),
    };
    ArchiveBuilder::new()
        .entry(dataforge)
        .entry(locales)
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, key)?;
    Ok((folder, archive))
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(SCOPE, ["FixturePlanet", "FixtureCluster"])
        .enumeration(BOARD, ["FixtureBoardOnly"])
        .structure(TestStruct::new(SCENARIO_TYPE))
        .structure(TestStruct::new(PARAMS).field("scope", TestType::Enum(SCOPE.to_owned())))
        .structure(
            TestStruct::new(AVAILABILITY)
                .field("missionBoardAvailability", TestType::Enum(BOARD.to_owned())),
        )
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
                .array("generators", TestType::StrongPointer(HANDLER.to_owned()))
                .array(REQUIRED, TestType::ReferenceTo(SCENARIO_TYPE.to_owned())),
        )
        .record(generator(PRIMARY, "primary", "FixturePlanet"))
        .record(generator(OTHER, "secondary", "FixtureCluster"))
        .record(TestRecord::new(
            SCENARIO,
            SCENARIO_TYPE,
            "MissionScenario.Fixture",
            "libs/foundry/records/missionscenarios/fixture.xml",
        ))
        .build()
}

fn generator(guid: u128, name: &str, scope: &str) -> TestRecord {
    let local = TestInstance::new(PARAMS).value("scope", TestValue::text(scope));
    let board = TestInstance::new(AVAILABILITY).value(
        "missionBoardAvailability",
        TestValue::text("FixtureBoardOnly"),
    );
    let handler = TestInstance::new(HANDLER)
        .value("localGeneratorParams", TestValue::Struct(local))
        .value("defaultAvailability", TestValue::Struct(board));
    TestRecord::new(
        guid,
        GENERATOR,
        format!("ContractGenerator.{name}"),
        format!("libs/foundry/records/contracts/contractgenerator/facets_{name}.xml"),
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
    .value(
        REQUIRED,
        TestValue::Array(vec![
            TestValue::Reference(SCENARIO),
            TestValue::Reference(0),
        ]),
    )
}
