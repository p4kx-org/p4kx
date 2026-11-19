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

use super::super::{FUEL_INTAKES, FUEL_TANKS, RESOURCE_CONTAINERS};
use super::{Route, TestResult};

pub(super) fn read() -> TestResult<[Route; 3]> {
    const TANK: &str = "SCItemFuelTankParams";
    const INTAKE: &str = "SCItemFuelIntakeParams";
    const CONTAINER: &str = "ResourceContainer";
    const REMAINING: &str = "installed fuel and resource container parity remains open";
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fuel_sdk_fixture.p4k");
    let database = [TANK, INTAKE, CONTAINER]
        .into_iter()
        .fold(
            entity_schema(DataForgeBuilder::new().version(8)),
            |builder, source| {
                builder.structure(TestStruct::new(source).parent("DataForgeComponentParams"))
            },
        )
        .record(
            TestEntity::new(
                1,
                "FixtureFuel",
                "libs/foundry/records/entities/scitem/ships/fuel/fixture.xml",
            )
            .attach_type("Misc")
            .component(TestValue::Struct(TestInstance::new(TANK)))
            .component(TestValue::Struct(TestInstance::new(INTAKE)))
            .component(TestValue::Struct(TestInstance::new(CONTAINER)))
            .record(),
        )
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let selection = CatalogRecordSelection::All;
    Ok([
        parsed.with_fuel_tanks(selection, None, |snapshot| {
            Route::queried_rows(FUEL_TANKS, snapshot.rows(), REMAINING)
        })??,
        parsed.with_fuel_intakes(selection, None, |snapshot| {
            Route::queried_rows(FUEL_INTAKES, snapshot.rows(), REMAINING)
        })??,
        parsed.with_resource_containers(selection, None, |snapshot| {
            Route::queried_rows(RESOURCE_CONTAINERS, snapshot.rows(), REMAINING)
        })??,
    ])
}
