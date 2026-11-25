// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::crafting::{CrafterDefinition, with_crafters_from};
use p4kx_export::catalog_queries::{CatalogRecordSelection, ResourceNumber};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::{Route, TestResult};
use crate::sdk_full_proof as full_proof;

pub(in super::super) const DATASET: &str = "/definitions/crafters";

const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "CrafterComponentParams";
const ROOT: &str = "libs/foundry/records/entities/basebuilding";
const GUID: u128 = 0x17;

pub(super) fn read() -> TestResult<Route> {
    let (_folder, archive) = open(&database())?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog_from(&archive, &entry, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let session = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?;
    session.with_crafters(CatalogRecordSelection::All, None, |snapshot| {
        let [first, second] = snapshot.rows.as_slice() else {
            return Err("exactly two admitted physical crafting owners".into());
        };
        verify_primary(first)?;
        assert_eq!(
            first.source().guid().value(),
            second.source().guid().value(),
            "nominal duplicates survive"
        );
        assert_eq!(
            second.source().record_index().value(),
            Some(&1),
            "physical order resolves the serialization tie"
        );
        assert_eq!(
            catalog.pointer(DATASET),
            Some(&json!({"availability": snapshot.availability, "rows": snapshot.rows})),
            "both canonical rows match the Catalog producer including source uses"
        );
        Route::queried_rows(
            DATASET,
            &snapshot.rows,
            "exact synthetic producer and scope parity available; installed producer parity open",
        )
    })?
}

#[test]
fn physical_station_selection_does_not_read_a_broken_neighbor() -> TestResult<()> {
    let broken = TestRecord::new(0x18, ENTITY, "Broken", format!("{ROOT}/broken.xml"))
        .with_instance_index(u16::MAX);
    let (_folder, archive) = open(&database().record(broken))?;
    let entry = DataForgeEntry::read(&archive)?;
    with_crafters_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| {
            assert!(
                snapshot.coverage.unreadable_bodies.is_empty(),
                "the unrelated broken owner is outside the selection"
            );
            let [row] = snapshot.rows.as_slice() else {
                return Err("one selected original owner".into());
            };
            verify_primary(row)
        },
    )?
}

fn verify_primary(row: &CrafterDefinition<'_>) -> TestResult<()> {
    assert_eq!(
        row.source().record_index().value(),
        Some(&0),
        "the selected physical owner"
    );
    let component = row.component().value().ok_or("the authored component")?;
    assert_eq!(
        component.component_ordinal().value(),
        Some(&1),
        "the first exact component follows a null"
    );
    let limit = component.max_jobs_in_history();
    assert!(
        matches!(limit.value(), Some(ResourceNumber::UInt64 { value }) if value.get() == u64::MAX),
        "the first component's original full-width limit survives"
    );
    assert_eq!(
        limit.origin().contains_encrypted_source(),
        Some(true),
        "the limit has its own encrypted source origin"
    );
    Ok(())
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(ENTITY).array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ))
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new(COMPONENT)
                .parent("DataForgeComponentParams")
                .field("maxJobsInHistory", TestType::UInt64),
        )
        .record(station(
            "first",
            [TestValue::Null, component(u64::MAX), component(2)],
        ))
        .record(station(
            "second",
            [component(3), TestValue::Null, TestValue::Null],
        ))
}

fn station(name: &str, values: [TestValue; 3]) -> TestRecord {
    TestRecord::new(GUID, ENTITY, name, format!("{ROOT}/{name}.xml"))
        .value("Components", TestValue::Array(Vec::from(values)))
}

fn component(limit: u64) -> TestValue {
    TestValue::Struct(
        TestInstance::new(COMPONENT).value("maxJobsInHistory", TestValue::UInt64(limit)),
    )
}

fn open(database: &DataForgeBuilder) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("synthetic-crafters.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    Ok((directory, Archive::open_with_key(path, Some(test_key()))?))
}
