// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::{
    InventoryContainerSnapshot, with_inventory_containers, with_inventory_containers_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries,
};
use p4kx_export::{CatalogError, CatalogOptions, build_catalog_from};
use p4kx_extraction::{
    Archive, CancelToken, DataForgeEntry, FieldOrigin, InputScope, Language, SourceEncryptionState,
    SourceInventory, SourceUseRole,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use super::super::{INVENTORY_CONTAINERS, PROVENANCE};
use super::{Route, TestResult};

#[path = "sdk_inventory_configuration_route.rs"]
mod configurations;
#[path = "../it/catalog_inventory_entities_fixture.rs"]
mod entities;
#[path = "../it/catalog_inventory_container_fixture.rs"]
mod fixture;

pub(super) fn read_configurations() -> TestResult<Route> {
    configurations::read()
}

const CONTAINER: &str = "InventoryContainer";
const UNRELATED: &str = "UnrelatedFixtureRecord";

#[derive(Clone, Copy)]
enum DataForgeStorage {
    Plain,
    Encrypted,
}

pub(super) fn read() -> TestResult<[Route; 2]> {
    let (_folder, archive) = open(&fixture::database(), DataForgeStorage::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    with_inventory_containers_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            check_catalog(&snapshot, &catalog)?;
            Ok([
                Route::queried_rows(
                    INVENTORY_CONTAINERS,
                    snapshot.rows(),
                    "synthetic source and producer parity witnessed; installed producer parity open",
                )?,
                Route::queried_value(
                    PROVENANCE,
                    snapshot.source_inventory(),
                    "reachable source and use parity verified on an encrypted fixture; broader installed parity open",
                )?,
            ])
        },
    )?
}

fn check_catalog(snapshot: &InventoryContainerSnapshot<'_>, catalog: &Value) -> TestResult {
    let table = json!({"availability": snapshot.availability(), "rows": snapshot.rows()});
    assert_eq!(
        Some(&table),
        catalog.pointer(INVENTORY_CONTAINERS),
        "SDK and Catalog retain the same ordered typed definitions and field origins"
    );
    assert_eq!(
        positions(snapshot)?,
        [0, 2, 3, 4, 5, 1],
        "GUID order differs from physical storage order"
    );
    assert!(
        snapshot.coverage().unreadable_headers.is_empty()
            && snapshot.coverage().unreadable_bodies.is_empty()
            && snapshot.coverage().incomplete_fields.is_empty(),
        "every selected definition projected from a readable original"
    );
    let catalog_uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("Catalog source uses")?;
    let catalog_sources = catalog
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("Catalog physical sources")?;
    for row in snapshot.rows() {
        let dimensions = row.interior_dimensions();
        let first_axis = dimensions
            .value()
            .ok_or("original interior dimensions")?
            .x();
        for (origin, role) in [
            (row.inventory_type().origin(), SourceUseRole::Selector),
            (dimensions.origin(), SourceUseRole::Selector),
            (first_axis.origin(), SourceUseRole::Payload),
        ] {
            check_origin(
                origin,
                role,
                snapshot.source_inventory(),
                catalog_uses,
                catalog_sources,
            )?;
        }
    }
    Ok(())
}

fn check_origin(
    origin: &FieldOrigin,
    role: SourceUseRole,
    inventory: &SourceInventory,
    catalog_uses: &[Value],
    catalog_sources: &[Value],
) -> TestResult {
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the original field requires the encrypted DataForge payload"
    );
    assert!(
        origin.source_set_complete(),
        "complete original field evidence"
    );
    let mut contributor = false;
    for reference in origin.source_uses_refs() {
        let used = inventory
            .iter_uses()
            .find(|used| used.id() == reference)
            .ok_or("SDK field contributor")?;
        contributor |= used.scope() == InputScope::Payload
            && used.role() == role
            && used.effective_encryption() == SourceEncryptionState::Encrypted;
        assert!(
            catalog_uses.contains(&serde_json::to_value(used)?),
            "the Catalog records the same original role and coordinate"
        );
        let physical = inventory
            .iter_sources()
            .find(|source| source.id() == used.source())
            .ok_or("the SDK contributor's physical source")?;
        assert!(
            catalog_sources.contains(&serde_json::to_value(physical)?),
            "the Catalog records the same physical source, ancestry and encryption facts",
        );
    }
    assert!(
        contributor,
        "original field lacks its encrypted {role:?} payload contributor"
    );
    Ok(())
}

#[test]
fn inventory_sdk_retains_physical_aliases_and_selects_only_requested_records() -> TestResult {
    let database = alias_database();
    let (_folder, archive) = open(&database, DataForgeStorage::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let guid = forge.record(0).ok_or("first inventory source")?.guid();
    let alias_index = parsed.with_inventory_containers(
        CatalogRecordSelection::Guid(guid),
        None,
        |snapshot| {
            let positions = positions(&snapshot)?;
            let [0, alias] = positions.as_slice() else {
                return Err("both original records sharing the GUID, in physical order".into());
            };
            assert!(
                snapshot.requested_languages().is_empty(),
                "no locale requested by this family"
            );
            assert!(
                snapshot.available_languages().is_empty(),
                "no locale opened by this family"
            );
            Ok::<_, Box<dyn std::error::Error>>(*alias)
        },
    )??;
    parsed.with_inventory_containers(
        CatalogRecordSelection::RecordIndex(alias_index),
        None,
        |snapshot| {
            assert_eq!(
                positions(&snapshot)?,
                [alias_index],
                "this physical alias alone"
            );
            Ok::<_, Box<dyn std::error::Error>>(())
        },
    )??;
    let other = forge
        .records()
        .find(|source| source.type_name().ok() == Some(UNRELATED))
        .ok_or("unrelated source Record")?;
    check_other_record(&parsed, other.index())?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "encrypted locale was never attempted"
    );
    Ok(())
}

fn check_other_record(parsed: &ParsedCatalogQueries<'_>, index: u32) -> TestResult {
    parsed.with_inventory_containers(
        CatalogRecordSelection::RecordIndex(index),
        None,
        |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "out-of-family source is excluded"
            );
            assert!(
                matches!(snapshot.availability(), DatasetAvailability::Available),
                "declared family remains available"
            );
        },
    )?;
    Ok(())
}

fn alias_database() -> DataForgeBuilder {
    fixture::database()
        .structure(TestStruct::new(UNRELATED))
        .record(TestRecord::new(
            999,
            UNRELATED,
            "Other",
            "fixture/other.xml",
        ))
        .record(
            TestRecord::new(
                fixture::OPEN,
                CONTAINER,
                "InventoryContainer.FixtureAlias",
                "fixture/alias.xml",
            )
            .value("interiorDimensions", fixture::vec3([9.0, 8.0, 7.0])),
        )
}

#[test]
fn inventory_sdk_preserves_incomplete_fields_and_cancellation() -> TestResult {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(CONTAINER).field("interiorDimensions", TestType::Int32))
        .record(
            TestRecord::new(
                fixture::OPEN,
                CONTAINER,
                "InventoryContainer.Invalid",
                "fixture/invalid.xml",
            )
            .value("interiorDimensions", TestValue::Int32(27)),
        );
    let (_folder, archive) = open(&database, DataForgeStorage::Plain)?;
    with_inventory_containers(&archive, CatalogRecordSelection::All, None, |snapshot| {
        let [row] = snapshot.rows() else {
            return Err("one readable but incomplete inventory definition".into());
        };
        assert!(
            row.interior_dimensions().value().is_none(),
            "scalar cannot become a vector"
        );
        assert!(
            !row.additional_fields().roots().is_empty(),
            "the incompatible original remains retained"
        );
        assert_eq!(
            snapshot.coverage().incomplete_fields,
            [0],
            "exact incomplete owner"
        );
        Ok::<_, Box<dyn std::error::Error>>(())
    })??;
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(
        matches!(
            with_inventory_containers(&archive, CatalogRecordSelection::All, Some(&cancel), |_| ()),
            Err(CatalogError::Cancelled)
        ),
        "a cancelled query does not deliver a snapshot"
    );
    Ok(())
}

fn positions(snapshot: &InventoryContainerSnapshot<'_>) -> TestResult<Vec<u32>> {
    snapshot
        .rows()
        .iter()
        .map(|row| {
            row.source()
                .record_index()
                .value()
                .copied()
                .ok_or_else(|| "original physical owner".into())
        })
        .collect()
}

fn open(
    database: &DataForgeBuilder,
    storage: DataForgeStorage,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("inventory-fixture.p4k");
    let dataforge = TestEntry::stored("Data/Game2.dcb", database.build());
    let (dataforge, key) = match storage {
        DataForgeStorage::Plain => (dataforge, None),
        DataForgeStorage::Encrypted => (dataforge.encrypted(), Some(p4kx_testkit::test_key())),
    };
    ArchiveBuilder::new()
        .entry(dataforge)
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), b"unused=fixture\n").encrypted(),
        )
        .write_to(&path)?;
    Ok((folder, Archive::open_with_key(path, key)?))
}
