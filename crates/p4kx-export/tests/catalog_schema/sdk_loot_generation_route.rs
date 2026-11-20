// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::loot_generation::with_loot_generation_components_from;
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestStruct, TestType,
    TestValue, entity_schema,
};
use serde_json::Value;

use super::super::LOOT_GENERATION;
use super::component_parity::parity;
use super::{Route, TestResult};

const COMPONENT: &str = "LootGenerationComponentParams";

pub(super) fn read() -> TestResult<Route> {
    let (_directory, archive) = fixture()?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions::new(UNIX_EPOCH);
    let catalog: Value =
        serde_json::from_slice(build_catalog_from(&archive, &entry, &options)?.json())?;
    with_loot_generation_components_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            parity(LOOT_GENERATION, &snapshot, &catalog)?;
            Route::queried_rows(
                LOOT_GENERATION,
                snapshot.rows(),
                "synthetic producer parity witnessed; installed producer parity open",
            )
        },
    )?
}

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("loot_generation.p4k");
    let database = entity_schema(DataForgeBuilder::new().version(8))
        .structure(
            TestStruct::new(COMPONENT)
                .parent("DataForgeComponentParams")
                .field("fixtureWeight", TestType::UInt32),
        )
        .record(
            TestEntity::new(1, "FixtureLoot", "synthetic-outside-catalog/fixture.xml")
                .attach_type("Misc")
                .component(TestValue::Struct(
                    TestInstance::new(COMPONENT).value("fixtureWeight", TestValue::UInt32(3)),
                ))
                .record(),
        )
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database))
        .write_to(&path)?;
    Ok((directory, Archive::open(path)?))
}
