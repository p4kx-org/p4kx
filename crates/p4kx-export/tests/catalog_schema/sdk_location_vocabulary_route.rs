// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::location_vocabulary::{
    LocationVocabularyFamily as Family, LocationVocabularyRows, LocationVocabularySnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    with_location_entity_declarations,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, Language, SourceLocation, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
    test_key,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::super::LOCATION_ENTITIES;
use super::{Route, TestResult};

#[path = "../it/location_vocabulary_fixture.rs"]
mod fixture;

const DATASETS: [(Family, &str); 7] = [
    (Family::Classes, "/definitions/locationClasses"),
    (Family::Functions, "/definitions/locationFunctions"),
    (Family::Sizes, "/definitions/locationSizes"),
    (Family::ResourceSlots, "/definitions/locationResourceSlots"),
    (
        Family::ClassRegistries,
        "/definitions/locationRegistries/classes",
    ),
    (
        Family::FunctionRegistries,
        "/definitions/locationRegistries/functions",
    ),
    (
        Family::SizeRegistries,
        "/definitions/locationRegistries/sizes",
    ),
];

pub(super) fn datasets() -> impl Iterator<Item = &'static str> {
    DATASETS.into_iter().map(|(_, dataset)| dataset)
}

pub(super) fn read() -> TestResult<Vec<Route>> {
    let (_directory, archive) = open(DataForgeStorage::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    DATASETS
        .into_iter()
        .map(|(family, path)| {
            parsed.with_location_vocabulary(
                family,
                CatalogRecordSelection::All,
                None,
                |snapshot| verify_family(path, &snapshot, &catalog),
            )?
        })
        .collect()
}

fn verify_family(
    path: &'static str,
    snapshot: &LocationVocabularySnapshot<'_>,
    catalog: &Value,
) -> TestResult<Route> {
    let uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("canonical source use inventory")?;
    for used in snapshot.source_inventory.iter_uses() {
        assert!(
            uses.contains(&serde_json::to_value(used)?),
            "each query use retains its exact canonical catalog counterpart"
        );
    }
    match &snapshot.rows {
        LocationVocabularyRows::Classes(rows) => {
            compare(path, snapshot.availability, rows, catalog)
        }
        LocationVocabularyRows::Functions(rows) | LocationVocabularyRows::Sizes(rows) => {
            compare(path, snapshot.availability, rows, catalog)
        }
        LocationVocabularyRows::ResourceSlots(rows) => {
            let [slot] = rows.as_slice() else {
                return Err("one original resource slot".into());
            };
            let count = slot.number_of_resource_slots_per_entity();
            assert_eq!(
                count.value(),
                Some(&3),
                "resource slot count keeps its signed integer"
            );
            assert_eq!(
                count.origin().contains_encrypted_source(),
                Some(true),
                "resource slot count is encrypted at source"
            );
            compare(path, snapshot.availability, rows, catalog)
        }
        LocationVocabularyRows::ClassRegistries(rows)
        | LocationVocabularyRows::FunctionRegistries(rows)
        | LocationVocabularyRows::SizeRegistries(rows) => {
            compare(path, snapshot.availability, rows, catalog)
        }
        _ => Err("unrecognized typed vocabulary family".into()),
    }
}

fn compare<Row: JsonSchema + Serialize>(
    path: &'static str,
    availability: DatasetAvailability,
    rows: &[Row],
    catalog: &Value,
) -> TestResult<Route> {
    let query = json!({"availability": availability, "rows": rows});
    assert_eq!(
        catalog.pointer(path),
        Some(&query),
        "selected {path} rows use the canonical schema and field origins"
    );
    Route::queried_rows(
        path,
        rows,
        concat!(
            "synthetic producer parity verified. Standalone cold memory verified on LIVE 13220821 ",
            "and PTU 13260598. Complete installed Normal parity remains open",
        ),
    )
}

#[test]
fn vocabulary_sdk_keeps_original_registry_slots_without_opening_unselected_bodies() -> TestResult<()>
{
    let (_directory, archive) = open(DataForgeStorage::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let class = forge.record(1).ok_or("original class Record")?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    for (selection, expected) in [
        (CatalogRecordSelection::Guid(class.guid()), Some(1)),
        (CatalogRecordSelection::RecordIndex(0), Some(0)),
        (CatalogRecordSelection::RecordIndex(2), None),
    ] {
        parsed.with_location_vocabulary(Family::Classes, selection, None, |snapshot| {
            verify_class(&snapshot, expected)
        })??;
    }
    parsed.with_location_vocabulary(
        Family::ClassRegistries,
        CatalogRecordSelection::RecordIndex(4),
        None,
        |snapshot| verify_registry(&snapshot),
    )??;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "vocabulary never attempts the inaccessible locale"
    );
    Ok(())
}

fn verify_class(
    snapshot: &LocationVocabularySnapshot<'_>,
    expected: Option<u32>,
) -> TestResult<()> {
    let LocationVocabularyRows::Classes(rows) = &snapshot.rows else {
        return Err("typed class selection".into());
    };
    let positions = rows
        .iter()
        .map(|row| {
            row.source()
                .record_index()
                .value()
                .copied()
                .ok_or("physical class owner")
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert!(
        positions.iter().copied().eq(expected),
        "class query reads only the selected physical class"
    );
    assert!(
        matches!(snapshot.availability, DatasetAvailability::Available),
        "selection does not erase declared class family"
    );
    for used in snapshot.source_inventory.iter_uses() {
        if let SourceLocation::DataForge { record_index, .. } = used.location() {
            assert_eq!(
                Some(*record_index),
                expected,
                "the selected class inventory excludes sibling owners"
            );
        }
    }
    Ok(())
}

fn verify_registry(snapshot: &LocationVocabularySnapshot<'_>) -> TestResult<()> {
    let LocationVocabularyRows::ClassRegistries(rows) = &snapshot.rows else {
        return Err("typed class register selection".into());
    };
    let [row] = rows.as_slice() else {
        return Err("one selected register".into());
    };
    let [second, dangling, first, repeated, null] = row
        .members()
        .value()
        .ok_or("original register members")?
        .as_slice()
    else {
        return Err("five original member positions".into());
    };
    for (cell, expected) in [
        (second, Some(0)),
        (dangling, None),
        (first, Some(1)),
        (repeated, Some(1)),
    ] {
        assert_eq!(
            cell.value().ok_or("authored Reference")?.resolved_index(),
            expected,
            "original register order and target state"
        );
    }
    assert_eq!(
        null.state(),
        SourceValueState::Null,
        "the original null position remains"
    );
    for used in snapshot.source_inventory.iter_uses() {
        if let SourceLocation::DataForge {
            record_index,
            field,
        } = used.location()
        {
            assert!(
                *record_index == 4
                    || (matches!(*record_index, 0 | 1) && !field.contains("scopesVisibleTo")),
                "registry retains member headers without visiting their class bodies or unrelated Records"
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum DataForgeStorage {
    Plain,
    Encrypted,
}

fn database() -> DataForgeBuilder {
    const SLOT: &str = "LocationResourceSlot";
    const COUNT: &str = "numberOfResourceSlotsPerEntity";
    fixture::template_database("MissionLocationData")
        .structure(TestStruct::new(SLOT).field(COUNT, TestType::Int32))
        .record(
            TestRecord::new(
                0x1086_2000,
                SLOT,
                "LocationResourceSlot.Fixture",
                "libs/foundry/records/fixture_location/resource_slots/fixture.xml",
            )
            .value(COUNT, TestValue::Int32(3)),
        )
}

fn open(storage: DataForgeStorage) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("location-vocabulary.p4k");
    let dcb = TestEntry::stored("Data/Game2.dcb", database().build());
    let english = TestEntry::stored(Language::English.global_ini_path(), b"fixture=Unrelated\n");
    let (dataforge, english, key) = match storage {
        DataForgeStorage::Plain => (dcb, english.encrypted(), None),
        DataForgeStorage::Encrypted => (dcb.encrypted(), english, Some(test_key())),
    };
    ArchiveBuilder::new()
        .entry(dataforge)
        .entry(english)
        .write_to(&path)?;
    Ok((directory, Archive::open_with_key(path, key)?))
}

pub(super) fn read_entities(archive: &Archive) -> TestResult<Route> {
    with_location_entity_declarations(
        archive,
        CatalogRecordSelection::All,
        None,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| Route::queried_rows(LOCATION_ENTITIES, &snapshot.rows, None),
    )?
}
