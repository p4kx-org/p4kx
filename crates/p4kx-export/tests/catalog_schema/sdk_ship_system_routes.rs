// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestRecord, TestStruct,
    TestType, TestValue, entity_schema,
};

use super::super::{
    JUMP_DRIVE_FLIGHT_TUNING, JUMP_DRIVES, QUANTUM_DRIVES, QUANTUM_INTERDICTION, SHIELD_GENERATORS,
};
use super::{Route, TestResult};

#[path = "sdk_ship_base_facts_route.rs"]
mod base_facts;

pub(super) fn read() -> TestResult<[Route; 6]> {
    const REMAINING: &str = "typed Item query and synthetic producer parity witnessed; complete installed producer parity open";
    const TUNING_REMAINING: &str = "typed tuning Record query and synthetic producer parity witnessed; complete installed producer parity open";
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("ship_system_fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database().build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let selected = CatalogRecordSelection::All;
    Ok([
        base_facts::read()?,
        parsed.with_shield_generators(selected, None, |snapshot| {
            Route::queried_rows(SHIELD_GENERATORS, snapshot.rows(), REMAINING)
        })??,
        parsed.with_quantum_drives(selected, None, |snapshot| {
            Route::queried_rows(QUANTUM_DRIVES, snapshot.rows(), REMAINING)
        })??,
        parsed.with_quantum_interdiction_generators(selected, None, |snapshot| {
            Route::queried_rows(QUANTUM_INTERDICTION, snapshot.rows(), REMAINING)
        })??,
        parsed.with_jump_drives(selected, None, |snapshot| {
            Route::queried_rows(JUMP_DRIVES, snapshot.rows(), REMAINING)
        })??,
        parsed.with_jump_drive_flight_tuning(selected, None, |snapshot| {
            Route::queried_rows(JUMP_DRIVE_FLIGHT_TUNING, snapshot.rows(), TUNING_REMAINING)
        })??,
    ])
}

fn database() -> DataForgeBuilder {
    const SHIELD: &str = "SCItemShieldGeneratorParams";
    const QUANTUM: &str = "SCItemQuantumDriveParams";
    const INTERDICTION: &str = "SCItemQuantumInterdictionGeneratorParams";
    const JUMP: &str = "SCItemJumpDriveParams";
    const TUNING: &str = "JumpDriveFlightParams";
    const TUNING_GUID: u128 = 2;
    entity_schema(DataForgeBuilder::new().version(8))
        .structure(TestStruct::new(SHIELD).parent("DataForgeComponentParams"))
        .structure(TestStruct::new(QUANTUM).parent("DataForgeComponentParams"))
        .structure(
            TestStruct::new(INTERDICTION)
                .parent("DataForgeComponentParams")
                .field("basePowerDrawFraction", TestType::Single),
        )
        .structure(
            TestStruct::new(JUMP)
                .parent("DataForgeComponentParams")
                .field("flightTuning", TestType::ReferenceTo(TUNING.to_owned())),
        )
        .structure(TestStruct::new(TUNING).field("respoolTime", TestType::Single))
        .record(
            TestEntity::new(
                1,
                "FixtureShipSystems",
                "libs/foundry/records/entities/scitem/ships/fixture_systems.xml",
            )
            .attach_type("Misc")
            .component(TestValue::Struct(TestInstance::new(SHIELD)))
            .component(TestValue::Struct(TestInstance::new(QUANTUM)))
            .component(TestValue::Struct(
                TestInstance::new(INTERDICTION)
                    .value("basePowerDrawFraction", TestValue::Single(0.5)),
            ))
            .component(TestValue::Struct(
                TestInstance::new(JUMP).value("flightTuning", TestValue::Reference(TUNING_GUID)),
            ))
            .record(),
        )
        .record(
            TestRecord::new(
                TUNING_GUID,
                TUNING,
                "FixtureFlightTuning",
                "fixture/jump_tuning.xml",
            )
            .value("respoolTime", TestValue::Single(2.0)),
        )
}
