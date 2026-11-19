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

use super::super::{RESOURCE_COMPONENTS, RESOURCE_CONNECTIONS};
use super::{Route, TestResult};

pub(super) fn read() -> TestResult<[Route; 2]> {
    const COMPONENT: &str = "ItemResourceComponentParams";
    const POWER: &str = "EntityComponentPowerConnection";
    const HEAT: &str = "EntityComponentHeatConnection";
    const REMAINING: &str = "installed resource component and connection parity remains open";
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("resource_network_fixture.p4k");
    let database = [COMPONENT, POWER, HEAT]
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
                "FixtureResource",
                "libs/foundry/records/entities/scitem/ships/resource/fixture.xml",
            )
            .attach_type("Misc")
            .component(TestValue::Struct(TestInstance::new(COMPONENT)))
            .component(TestValue::Struct(TestInstance::new(POWER)))
            .component(TestValue::Struct(TestInstance::new(HEAT)))
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
    Ok([
        parsed.with_resource_components(CatalogRecordSelection::All, None, |snapshot| {
            Route::queried_rows(RESOURCE_COMPONENTS, snapshot.rows(), REMAINING)
        })??,
        parsed.with_resource_connections(CatalogRecordSelection::All, None, |snapshot| {
            Route::queried_rows(RESOURCE_CONNECTIONS, snapshot.rows(), REMAINING)
        })??,
    ])
}
