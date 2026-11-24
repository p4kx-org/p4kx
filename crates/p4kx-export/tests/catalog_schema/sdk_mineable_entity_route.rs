// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{CatalogRecordSelection, with_mineable_entities};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestStruct, TestValue,
    entity_schema,
};

use super::super::MINEABLE_ENTITIES;
use super::{Route, TestResult};

pub(super) fn read() -> TestResult<Route> {
    const MINING: &str = "MineableParams";
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("mineable_entity_fixture.p4k");
    let database = entity_schema(DataForgeBuilder::new().version(8))
        .structure(TestStruct::new(MINING).parent("DataForgeComponentParams"))
        .record(
            TestEntity::new(
                1,
                "FixtureMineable",
                "libs/foundry/records/entities/mineable/fixture_mineable",
            )
            .component(TestValue::Struct(TestInstance::new(MINING)))
            .record(),
        )
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    with_mineable_entities(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| Route::queried_rows(MINEABLE_ENTITIES, snapshot.rows(), None),
    )?
}
