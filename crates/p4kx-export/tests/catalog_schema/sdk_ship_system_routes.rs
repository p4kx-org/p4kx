// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestStruct, TestValue,
    entity_schema,
};

use super::super::{QUANTUM_DRIVES, SHIELD_GENERATORS};
use super::{Route, TestResult};

pub(super) fn read() -> TestResult<[Route; 2]> {
    const SHIELD: &str = "SCItemShieldGeneratorParams";
    const QUANTUM: &str = "SCItemQuantumDriveParams";
    const REMAINING: &str = "typed Item query and synthetic producer parity witnessed; complete installed producer parity open";
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("ship_system_fixture.p4k");
    let database = entity_schema(DataForgeBuilder::new().version(8))
        .structure(TestStruct::new(SHIELD).parent("DataForgeComponentParams"))
        .structure(TestStruct::new(QUANTUM).parent("DataForgeComponentParams"))
        .record(
            TestEntity::new(
                1,
                "FixtureShipSystems",
                "libs/foundry/records/entities/scitem/ships/fixture_systems.xml",
            )
            .attach_type("Misc")
            .component(TestValue::Struct(TestInstance::new(SHIELD)))
            .component(TestValue::Struct(TestInstance::new(QUANTUM)))
            .record(),
        );
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let selected = CatalogRecordSelection::All;
    Ok([
        parsed.with_shield_generators(selected, None, |snapshot| {
            Route::queried_rows(SHIELD_GENERATORS, snapshot.rows(), REMAINING)
        })??,
        parsed.with_quantum_drives(selected, None, |snapshot| {
            Route::queried_rows(QUANTUM_DRIVES, snapshot.rows(), REMAINING)
        })??,
    ])
}
