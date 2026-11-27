// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::env;

use p4kx_export::catalog_queries::npc_spawns::{NpcSpawnDefinitionFamily, NpcSpawnDefinitionRows};
use p4kx_export::catalog_queries::{
    CanonicalRecordHeader, CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, SourceInventory};
use serde::Serialize;
use serde_json::Value;

use super::equipment_input;
use crate::real_archive_catalog_table_parity::{self, Origins, Table, TestResult};
use crate::real_archive_verification;

#[derive(Clone, Copy)]
struct Family {
    kind: NpcSpawnDefinitionFamily,
    path: &'static str,
}

impl Family {
    const fn new(kind: NpcSpawnDefinitionFamily, path: &'static str) -> Self {
        Self { kind, path }
    }
}

const FAMILIES: [Family; 5] = [
    Family::new(
        NpcSpawnDefinitionFamily::Archetypes,
        "/definitions/npcArchetypes",
    ),
    Family::new(
        NpcSpawnDefinitionFamily::ClassPools,
        "/definitions/npcClassPools",
    ),
    Family::new(NpcSpawnDefinitionFamily::Spawns, "/definitions/autoSpawns"),
    Family::new(NpcSpawnDefinitionFamily::Sets, "/definitions/autoSpawnSets"),
    Family::new(
        NpcSpawnDefinitionFamily::Encounters,
        "/definitions/encounterIndices",
    ),
];

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_npc_spawn_definitions_match_one_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_NPC_SPAWN_DEFINITION_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact NPC spawn definition Catalog producer revision")?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &revision,
        |archive, measurement| {
            let (tables, origins) = retained_inputs(archive)?;
            let counts =
                measurement.verify_tables(FAMILIES.map(|family| family.path), &tables, &origins)?;
            real_archive_verification::print_line(&format!(
                "complete NPC spawn definition SDK parity: rows={:?} sourceUses={} physicalSources={}",
                counts.rows, counts.source_uses, counts.physical_sources,
            ));
            Ok(())
        },
    )
}

fn retained_inputs(archive: &Archive) -> TestResult<([Table; FAMILIES.len()], Origins)> {
    let session =
        equipment_input::full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
    let parsed = session.parsed(None)?;
    let mut origins = Origins::default();
    let mut tables = Vec::with_capacity(FAMILIES.len());
    for family in FAMILIES {
        let table = read_family(archive, &parsed, family, Some(&mut origins))?
            .ok_or("missing an NPC spawn definition parity table")?;
        tables.push(table);
    }
    let tables = tables
        .try_into()
        .map_err(|_| "expected every NPC spawn definition family")?;
    Ok((tables, origins))
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact dataset path"]
fn installed_npc_spawn_definition_cold_query() -> TestResult {
    let path = env::var("P4KX_NPC_SPAWN_DEFINITION_SDK_COLD_DATASET")
        .map_err(|_| "configure one canonical NPC spawn definition dataset path")?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path == path)
        .ok_or("expected one exact NPC spawn definition dataset path")?;
    equipment_input::with_archive(|archive| {
        let session =
            equipment_input::full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
        let parsed = session.parsed(None)?;
        let _ = read_family(archive, &parsed, family, None)?;
        Ok(())
    })
}

fn read_family(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    family: Family,
    origins: Option<&mut Origins>,
) -> TestResult<Option<Table>> {
    parsed.with_npc_spawn_definitions(
        family.kind,
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            if snapshot.build.is_none() || snapshot.build.as_ref() != archive.build() {
                return Err(
                    format!("{}: NPC spawn definition Build does not match", family.path).into(),
                );
            }
            let witness = QueryWitness {
                path: family.path,
                availability: snapshot.availability,
                inventory: &snapshot.source_inventory,
                coverage: &snapshot.coverage,
                origins,
            };
            match &snapshot.rows {
                NpcSpawnDefinitionRows::Archetypes(rows) => {
                    witness.inspect(rows, |row| physical_index(&row.source))
                }
                NpcSpawnDefinitionRows::ClassPools(rows) => {
                    witness.inspect(rows, |row| physical_index(&row.source))
                }
                NpcSpawnDefinitionRows::Spawns(rows) => {
                    witness.inspect(rows, |row| physical_index(&row.source))
                }
                NpcSpawnDefinitionRows::Sets(rows) => {
                    witness.inspect(rows, |row| physical_index(&row.source))
                }
                NpcSpawnDefinitionRows::Encounters(rows) => {
                    witness.inspect(rows, |row| physical_index(&row.source))
                }
                _ => Err("unsupported NPC spawn definition SDK family".into()),
            }
        },
    )?
}

struct QueryWitness<'scope> {
    path: &'static str,
    availability: DatasetAvailability,
    inventory: &'scope SourceInventory,
    coverage: &'scope CatalogRecordCoverage,
    origins: Option<&'scope mut Origins>,
}

impl QueryWitness<'_> {
    fn inspect<Row: Serialize>(
        self,
        rows: &[Row],
        identity: impl Fn(&Row) -> TestResult<u32>,
    ) -> TestResult<Option<Table>> {
        require_complete(self.path, self.availability, rows.len(), self.coverage)?;
        let uses = self.inventory.use_count();
        let sources = self.inventory.source_count();
        if !rows.is_empty() && (uses == 0 || sources == 0) {
            return Err("NPC spawn definition rows have an empty original source inventory".into());
        }
        real_archive_verification::print_line(&format!(
            "NPC spawn definition SDK: dataset={} availability={:?} rows={} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
            self.path,
            self.availability,
            rows.len(),
            self.coverage.skipped_leftovers,
        ));
        if let Some(origins) = self.origins {
            return real_archive_catalog_table_parity::capture_rows(
                self.availability,
                rows,
                self.inventory,
                origins,
                wire_physical_index,
            )
            .map(Some);
        }
        let mut physical = HashSet::with_capacity(rows.len());
        for row in rows {
            if !physical.insert(identity(row)?) {
                return Err("the NPC spawn definition cold query repeats a physical Record".into());
            }
        }
        Ok(None)
    }
}

fn physical_index(source: &CanonicalRecordHeader<'_>) -> TestResult<u32> {
    source
        .record_index()
        .value()
        .copied()
        .ok_or_else(|| "expected an original NPC spawn definition Record index".into())
}

fn wire_physical_index(row: &Value) -> TestResult<u32> {
    row.pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| "expected an original NPC spawn definition Record index".into())
}

fn require_complete(
    path: &str,
    availability: DatasetAvailability,
    rows: usize,
    coverage: &CatalogRecordCoverage,
) -> TestResult {
    match availability {
        DatasetAvailability::Available => {}
        DatasetAvailability::Absent if rows == 0 => {}
        _ => {
            return Err(
                format!("{path}: expected complete NPC spawn declaration availability").into(),
            );
        }
    }
    if !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err(format!(
            "{path}: incomplete NPC spawn coverage (unreadableHeaders={} unreadableBodies={} incompleteFields={})",
            coverage.unreadable_headers.len(), coverage.unreadable_bodies.len(), coverage.incomplete_fields.len(),
        )
        .into());
    }
    Ok(())
}

#[test]
fn npc_spawn_definition_parity_requires_complete_physical_coverage() {
    let [family, ..] = FAMILIES;
    let path = family.path;
    let mut coverage = CatalogRecordCoverage::default();
    for (availability, rows) in [
        (DatasetAvailability::Available, 1),
        (DatasetAvailability::Available, 0),
        (DatasetAvailability::Absent, 0),
    ] {
        assert!(
            require_complete(path, availability, rows, &coverage).is_ok(),
            "{availability:?} with {rows} complete rows is a valid disposition",
        );
    }
    for (availability, rows) in [
        (DatasetAvailability::Absent, 1),
        (DatasetAvailability::Unavailable, 0),
    ] {
        assert!(
            require_complete(path, availability, rows, &coverage).is_err(),
            "{availability:?} with {rows} rows does not prove a complete disposition",
        );
    }
    coverage.unreadable_headers.push(0);
    assert!(
        require_complete(path, DatasetAvailability::Absent, 0, &coverage).is_err(),
        "an unreadable header cannot establish absence",
    );
    coverage.unreadable_headers.clear();
    coverage.unreadable_bodies.push(0);
    assert!(
        require_complete(path, DatasetAvailability::Available, 1, &coverage).is_err(),
        "an unreadable body cannot establish complete coverage",
    );
    coverage.unreadable_bodies.clear();
    coverage.incomplete_fields.push(0);
    assert!(
        require_complete(path, DatasetAvailability::Available, 1, &coverage).is_err(),
        "an incompletely projected original field cannot establish complete coverage",
    );
}
