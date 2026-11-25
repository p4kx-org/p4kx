// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::cargo_manifests::{
    CargoManifestDefinition, CargoManifestSnapshot, with_cargo_manifests, with_cargo_manifests_from,
};
use p4kx_export::catalog_queries::equipment::cargo_globals;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ResourceNumber,
};
use p4kx_export::{CatalogError, CatalogOptions, build_catalog_from};
use p4kx_extraction::{
    Archive, CancelToken, DataForgeEntry, FieldOrigin, Language, SourceLocation,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::super::{CARGO_LOADING_GLOBALS, CARGO_MANIFESTS, SHOP_COMMODITY_GLOBALS};
use super::{Route, TestResult};

const MANIFEST: &str = "CargoManifest";
const FILL: &str = "CargoFillCapacityValue_Even";
const RESOURCE: &str = "CargoResource";
const SOURCE_RESOURCE: &str = "ResourceType";
const GUID: u128 = 0x17;
const RESOURCE_GUID: u128 = 0x21;
const RESOURCE_INDEX: u32 = 4;

#[derive(Clone, Copy)]
enum DataForgeStorage {
    Plain,
    Encrypted,
}

pub(super) fn read_manifests() -> TestResult<Route> {
    let (_directory, archive) = open(&manifest_database(), DataForgeStorage::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let built = build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    with_cargo_manifests_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            verify_parity(&snapshot, &catalog)?;
            Route::queried_rows(CARGO_MANIFESTS, &snapshot.rows, None)
        },
    )?
}

fn verify_parity(snapshot: &CargoManifestSnapshot<'_>, catalog: &Value) -> TestResult<()> {
    let table = json!({"availability": snapshot.availability, "rows": snapshot.rows});
    assert_eq!(
        catalog.pointer(CARGO_MANIFESTS),
        Some(&table),
        "query shares the canonical row and origins"
    );
    verify_selection(snapshot, &[0, 1])?;
    let [first, second] = snapshot.rows.as_slice() else {
        return Err("two physical rows for the aliased CargoManifest".into());
    };
    for (row, authored_lock) in [(first, false), (second, true)] {
        assert_eq!(
            row.lock_cargo_grids_on_spawn().value(),
            Some(&authored_lock),
            "physical owner retains its distinct lock value"
        );
        verify_resource(row, snapshot)?;
    }
    let original_uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("producer source use inventory")?;
    for used in snapshot.source_inventory.iter_uses() {
        assert!(
            original_uses.contains(&serde_json::to_value(used)?),
            "each selected query use exists in the same-source producer inventory"
        );
    }
    Ok(())
}

fn verify_resource(
    row: &CargoManifestDefinition<'_>,
    snapshot: &CargoManifestSnapshot<'_>,
) -> TestResult<()> {
    let fill = row
        .cargo_fill_capacity()
        .value()
        .ok_or("original cargo fill")?;
    assert_eq!(
        fill.source_type().value(),
        Some(&FILL),
        "selected original filled cargo type"
    );
    let [resource] = fill
        .resources()
        .value()
        .ok_or("original resource slots")?
        .as_slice()
    else {
        return Err("one typed resource slot".into());
    };
    let resource = resource.value().ok_or("original direct resource")?;
    let reference = resource
        .resource()
        .value()
        .ok_or("authored ResourceType reference")?;
    assert_eq!(
        reference.resolved_index(),
        Some(RESOURCE_INDEX),
        "reference retains the linked physical Record"
    );
    assert_eq!(
        reference.target_type().value(),
        Some(&SOURCE_RESOURCE),
        "reference retains the declared target type"
    );
    let probability = resource.probability();
    let Some(ResourceNumber::Single(value)) = probability.value() else {
        return Err("authored Float32 probability".into());
    };
    assert_eq!(
        value.raw_bits(),
        0.25_f32.to_bits(),
        "exact original probability bits"
    );
    verify_probability_origin(probability.origin(), snapshot);
    Ok(())
}

fn verify_probability_origin(origin: &FieldOrigin, snapshot: &CargoManifestSnapshot<'_>) {
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the value comes from encrypted DataForge"
    );
    assert!(
        origin.source_set_complete(),
        "the authored probability has complete source evidence"
    );
    assert!(
        !origin.source_uses_refs().is_empty(),
        "there is source evidence to resolve"
    );
    for reference in origin.source_uses_refs() {
        assert!(
            snapshot
                .source_inventory
                .iter_uses()
                .any(|used| used.id() == reference),
            "each field reference resolves in this query inventory"
        );
    }
}

#[test]
fn cargo_manifest_sdk_uses_physical_selection_and_restricts_source_reads() -> TestResult<()> {
    let (_folder, archive) = open(&manifest_database(), DataForgeStorage::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let guid = forge.record(0).ok_or("manifest GUID")?.guid();
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    for (selection, expected) in [
        (CatalogRecordSelection::Guid(guid), [0, 1].as_slice()),
        (CatalogRecordSelection::RecordIndex(1), [1].as_slice()),
        (CatalogRecordSelection::RecordIndex(2), [].as_slice()),
        (CatalogRecordSelection::RecordIndex(3), [].as_slice()),
    ] {
        parsed.with_cargo_manifests(selection, None, |snapshot| {
            verify_selection(&snapshot, expected)
        })??;
    }
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "the inaccessible sibling locale was never opened"
    );
    Ok(())
}

fn verify_selection(snapshot: &CargoManifestSnapshot<'_>, expected: &[u32]) -> TestResult<()> {
    let positions = snapshot
        .rows
        .iter()
        .map(|row| {
            row.source()
                .record_index()
                .value()
                .copied()
                .ok_or("manifest physical record index")
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        positions, expected,
        "same GUID and path preserve physical row order, with the canonical family path filter"
    );
    assert!(
        matches!(snapshot.availability, DatasetAvailability::Available),
        "availability is independent of selected row count"
    );
    let coverage = &snapshot.coverage;
    assert!(
        coverage.incomplete_fields.is_empty()
            && coverage.unreadable_headers.is_empty()
            && coverage.unreadable_bodies.is_empty(),
        "fixture selected owners have no missing coverage"
    );
    for used in snapshot.source_inventory.iter_uses() {
        if let SourceLocation::DataForge {
            record_index,
            field,
        } = used.location()
        {
            assert!(
                expected.contains(record_index)
                    || (*record_index == RESOURCE_INDEX
                        && !field.contains("validateDefaultCargoBox")),
                "only selected manifest values and referenced target header facts are observed"
            );
        }
    }
    Ok(())
}

#[test]
fn cargo_manifest_sdk_reports_incompatible_fields_and_cancellation() -> TestResult<()> {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(MANIFEST).field("cargoFillCapacity", TestType::Int32))
        .record(
            TestRecord::new(
                GUID,
                MANIFEST,
                "CargoManifest.Invalid",
                "libs/foundry/records/cargomanifest/invalid.xml",
            )
            .value("cargoFillCapacity", TestValue::Int32(7)),
        );
    let (_folder, archive) = open(&database, DataForgeStorage::Plain)?;
    with_cargo_manifests(
        &archive,
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult<()> {
            let [row] = snapshot.rows.as_slice() else {
                return Err("incompatible member keeps its readable owner".into());
            };
            assert_eq!(
                snapshot.coverage.incomplete_fields,
                [0],
                "source error reports its physical owner"
            );
            assert!(
                row.cargo_fill_capacity().value().is_none(),
                "the scalar cannot stand in for an authored fill structure"
            );
            assert!(
                !row.authored_fields().roots().is_empty(),
                "the incompatible original member remains in the authored arena"
            );
            Ok(())
        },
    )??;
    let cancel = CancelToken::new();
    cancel.cancel();
    let cancelled =
        with_cargo_manifests(&archive, CatalogRecordSelection::All, Some(&cancel), |_| ());
    assert!(
        matches!(cancelled, Err(CatalogError::Cancelled)),
        "a cancelled read does not supply a snapshot"
    );
    Ok(())
}

fn manifest_database() -> DataForgeBuilder {
    let path = "libs/foundry/records/cargomanifest/fixture.xml";
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(MANIFEST)
                .field("lockCargoGridsOnSpawn", TestType::Bool)
                .field(
                    "cargoFillCapacity",
                    TestType::StrongPointer(FILL.to_owned()),
                ),
        )
        .structure(TestStruct::new(FILL).array("resources", TestType::Class(RESOURCE.to_owned())))
        .structure(
            TestStruct::new(RESOURCE)
                .field("resource", TestType::Reference)
                .field("probability", TestType::Single),
        )
        .structure(
            TestStruct::new("UnrelatedCargoFixture").field("unrelatedWitness", TestType::Int32),
        )
        .structure(
            TestStruct::new(SOURCE_RESOURCE).field("validateDefaultCargoBox", TestType::Bool),
        )
        .record(manifest(path, false))
        .record(manifest(path, true))
        .record(manifest(
            "libs/foundry/records/unrelated/manifest.xml",
            true,
        ))
        .record(
            TestRecord::new(
                GUID,
                "UnrelatedCargoFixture",
                "UnrelatedCargoFixture.Foreign",
                path,
            )
            .value("unrelatedWitness", TestValue::Int32(11)),
        )
        .record(
            TestRecord::new(
                RESOURCE_GUID,
                SOURCE_RESOURCE,
                "ResourceType.Fixture",
                "libs/foundry/records/resourcetypedatabase/fixture.xml",
            )
            .value("validateDefaultCargoBox", TestValue::Bool(true)),
        )
}

fn manifest(path: &str, locked: bool) -> TestRecord {
    TestRecord::new(GUID, MANIFEST, "CargoManifest.Fixture", path)
        .value("lockCargoGridsOnSpawn", TestValue::Bool(locked))
        .value(
            "cargoFillCapacity",
            TestValue::Struct(TestInstance::new(FILL).value(
                "resources",
                TestValue::Array(vec![
                TestValue::Struct(TestInstance::new(RESOURCE)
                    .value("resource", TestValue::Reference(RESOURCE_GUID))
                    .value("probability", TestValue::Single(0.25)))
            ]),
            )),
        )
}

fn open(
    database: &DataForgeBuilder,
    storage: DataForgeStorage,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("cargo-manifest-fixture.p4k");
    let dataforge = TestEntry::stored("Data/Game2.dcb", database.build());
    let english = TestEntry::stored(Language::English.global_ini_path(), b"fixture=Unrelated\n");
    let (dataforge, english, key) = match storage {
        DataForgeStorage::Plain => (dataforge, english.encrypted(), None),
        DataForgeStorage::Encrypted => (dataforge.encrypted(), english, Some(test_key())),
    };
    ArchiveBuilder::new()
        .entry(dataforge)
        .entry(english)
        .write_to(&path)?;
    Ok((folder, Archive::open_with_key(path, key)?))
}

pub(super) fn read_globals(archive: &Archive) -> TestResult<[Route; 2]> {
    let selection = CatalogRecordSelection::All;
    Ok([
        cargo_globals::with_cargo_loading_globals(archive, selection, None, |snapshot| {
            Route::queried_rows(CARGO_LOADING_GLOBALS, snapshot.rows(), None)
        })??,
        cargo_globals::with_shop_commodity_globals(archive, selection, None, |snapshot| {
            Route::queried_rows(SHOP_COMMODITY_GLOBALS, snapshot.rows(), None)
        })??,
    ])
}
