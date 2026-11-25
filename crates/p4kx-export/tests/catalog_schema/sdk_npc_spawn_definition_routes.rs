// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::npc_spawns::with_npc_spawn_definitions_from;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::npc_spawns::{
    NpcSpawnDefinitionFamily as Family, NpcSpawnDefinitionRows, NpcSpawnDefinitionSnapshot,
};
use p4kx_export::catalog_queries::{
    CanonicalRecordHeader, CatalogLanguageSelection, CatalogRecordSelection,
};
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, CancelToken, DataForgeEntry, FieldOrigin, InputScope, SourceInventory, SourceLocation,
};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestValue};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::{Route, TestResult};

#[path = "../it/catalog_auto_spawn_fixture.rs"]
mod auto_spawn_fixture;
#[path = "../it/catalog_npc_spawn_fixture.rs"]
mod npc_fixture;

const DATASETS: [(Family, &str); 5] = [
    (Family::Archetypes, "/definitions/npcArchetypes"),
    (Family::ClassPools, "/definitions/npcClassPools"),
    (Family::Spawns, "/definitions/autoSpawns"),
    (Family::Sets, "/definitions/autoSpawnSets"),
    (Family::Encounters, "/definitions/encounterIndices"),
];

pub(super) fn datasets() -> impl Iterator<Item = &'static str> {
    DATASETS.into_iter().map(|(_, path)| path)
}

pub(super) fn read() -> TestResult<Vec<Route>> {
    let (_folder, archive) = open(&auto_spawn_fixture::extend(npc_fixture::database()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, options.with_full_provenance())?.json(),
    )?;
    let session = full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    let mut routes = Vec::new();
    for (family, path) in DATASETS {
        routes.push(parsed.with_npc_spawn_definitions(
            family,
            CatalogRecordSelection::All,
            None,
            |snapshot| witness(&snapshot, path, &catalog),
        )??);
    }
    Ok(routes)
}

fn witness(
    snapshot: &NpcSpawnDefinitionSnapshot<'_>,
    path: &'static str,
    catalog: &Value,
) -> TestResult<Route> {
    let expected = catalog.pointer(path).ok_or("canonical NPC spawn table")?;
    let availability = snapshot.availability;
    let inventory = &snapshot.source_inventory;
    match &snapshot.rows {
        NpcSpawnDefinitionRows::Archetypes(rows) => {
            compare(path, rows, availability, expected, inventory, |row| {
                (&row.source, row.identity.origin())
            })
        }
        NpcSpawnDefinitionRows::ClassPools(rows) => {
            compare(path, rows, availability, expected, inventory, |row| {
                (&row.source, row.npc_classes.origin())
            })
        }
        NpcSpawnDefinitionRows::Spawns(rows) => {
            compare(path, rows, availability, expected, inventory, |row| {
                (&row.source, row.auto_spawn_data.origin())
            })
        }
        NpcSpawnDefinitionRows::Sets(rows) => {
            compare(path, rows, availability, expected, inventory, |row| {
                (&row.source, row.auto_spawn_data.origin())
            })
        }
        NpcSpawnDefinitionRows::Encounters(rows) => {
            compare(path, rows, availability, expected, inventory, |row| {
                (&row.source, row.encounters.origin())
            })
        }
        _ => Err("unrecognized SDK NPC spawn family".into()),
    }
}

fn compare<'source, Row: JsonSchema + Serialize>(
    dataset: &'static str,
    rows: &[Row],
    availability: p4kx_export::catalog_queries::DatasetAvailability,
    expected: &Value,
    inventory: &SourceInventory,
    source: impl for<'row> Fn(&'row Row) -> (&'row CanonicalRecordHeader<'source>, &'row FieldOrigin),
) -> TestResult<Route> {
    assert_eq!(
        &json!({"availability": availability, "rows": rows}),
        expected,
        "canonical NPC spawn table {dataset} matches query output"
    );
    for row in rows {
        let (header, field) = source(row);
        let index = header
            .record_index()
            .value()
            .copied()
            .ok_or("physical NPC Record index")?;
        assert_eq!(
            field.contains_encrypted_source(),
            Some(true),
            "the body field comes from the encrypted entry"
        );
        assert!(
            field.source_set_complete() && inventory.iter_uses().any(|used| {
                field.source_uses_refs().contains(used.id())
                    && used.scope() == InputScope::Payload
                    && matches!(used.location(), SourceLocation::Subresource { namespace, indices, .. }
                        if namespace == "dataforgeValue" && indices.first() == Some(&u64::from(index)))
            }),
            "the body field has evidence at its original physical NPC Record"
        );
    }
    Route::queried_rows(dataset, rows, None)
}

#[test]
fn npc_spawn_queries_preserve_duplicate_indices_and_exclude_other_families() -> TestResult {
    let guid = npc_fixture::POOL;
    let pool = |name: &str| {
        TestRecord::new(
            guid,
            "NPCClassPool",
            format!("NPCClassPool.{name}"),
            format!("libs/foundry/records/npc_fixture/pools/{name}.xml"),
        )
        .value("npcClasses", TestValue::Array(Vec::new()))
    };
    let database = npc_fixture::database().record(pool("duplicate")).record(
        TestRecord::new(
            guid,
            "NPCArchetype",
            "NPCArchetype.ForeignFamily",
            "libs/foundry/records/npc_fixture/archetypes/foreign.xml",
        )
        .value("identity", TestValue::Array(Vec::new())),
    );
    let (_folder, archive) = open(&database)?;
    let entry = DataForgeEntry::read(&archive)?;
    let guid = format!("00000000-0000-0000-0000-{guid:012x}").parse()?;
    let select = |family, selection| {
        full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
            .with_npc_spawn_definitions(family, selection, None, |snapshot| match snapshot.rows {
                NpcSpawnDefinitionRows::ClassPools(rows) => {
                    rows.iter().map(|row| &row.source).map(index).collect()
                }
                NpcSpawnDefinitionRows::Archetypes(rows) => {
                    rows.iter().map(|row| &row.source).map(index).collect()
                }
                _ => Err("unexpected selected NPC family".into()),
            })?
    };
    let canonical = ordinary_pool_indices(&archive, &entry, guid)?;
    verify_selection(select, guid, &canonical)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let cancelled = with_npc_spawn_definitions_from(
        &archive,
        &entry,
        Family::ClassPools,
        CatalogRecordSelection::All,
        Some(&cancel),
        |_| (),
    );
    assert!(
        matches!(cancelled, Err(CatalogError::Cancelled)),
        "cancellation does not deliver a snapshot"
    );
    Ok(())
}

fn verify_selection(
    read: impl Fn(Family, CatalogRecordSelection) -> TestResult<Vec<u32>>,
    guid: p4kx_extraction::Guid,
    canonical: &[u32],
) -> TestResult {
    let selection = CatalogRecordSelection::Guid(guid);
    let pools = read(Family::ClassPools, selection)?;
    let [first, second] = pools.as_slice() else {
        return Err("both physical pools sharing the GUID".into());
    };
    assert!(first < second, "equal GUIDs stay ordered by physical index");
    assert_eq!(
        pools, canonical,
        "ordinary Catalog and GUID-scoped SDK preserve the same physical pool rows"
    );
    let aliases = read(Family::Archetypes, selection)?;
    let [foreign] = aliases.as_slice() else {
        return Err("one same-GUID archetype outside the selected pool family".into());
    };
    assert_eq!(
        read(
            Family::ClassPools,
            CatalogRecordSelection::RecordIndex(*second)
        )?,
        [*second],
        "RecordIndex selects the second physical pool"
    );
    assert!(
        read(
            Family::ClassPools,
            CatalogRecordSelection::RecordIndex(*foreign)
        )?
        .is_empty(),
        "a physical index owned by a different family supplies no row"
    );
    Ok(())
}

fn ordinary_pool_indices(
    archive: &Archive,
    entry: &DataForgeEntry,
    guid: p4kx_extraction::Guid,
) -> TestResult<Vec<u32>> {
    let built = build_catalog_from(
        archive,
        entry,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let guid = json!(guid.to_string());
    catalog
        .pointer("/definitions/npcClassPools/rows")
        .and_then(Value::as_array)
        .ok_or("ordinary NPC class pool rows")?
        .iter()
        .filter(|row| row.pointer("/guid/value") == Some(&guid))
        .map(|row| {
            let position = row
                .pointer("/recordIndex/value")
                .and_then(Value::as_u64)
                .ok_or("physical pool Record index in the ordinary Catalog")?;
            Ok(u32::try_from(position)?)
        })
        .collect()
}

fn index(header: &CanonicalRecordHeader<'_>) -> TestResult<u32> {
    header
        .record_index()
        .value()
        .copied()
        .ok_or_else(|| "physical NPC Record index".into())
}

fn open(database: &DataForgeBuilder) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("npc-spawn.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}
