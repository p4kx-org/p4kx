// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::loot::{LootDefinitionKind, LootDefinitionSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries,
};
use p4kx_extraction::Archive;
use serde_json::Value;

use super::equipment_input;
use crate::real_archive_catalog_table_parity::{self, Origins, Table, TestResult};
use crate::real_archive_verification;

#[derive(Clone, Copy)]
struct Family {
    kind: LootDefinitionKind,
    path: &'static str,
}

impl Family {
    const fn new(kind: LootDefinitionKind, path: &'static str) -> Self {
        Self { kind, path }
    }
}

const FAMILIES: [Family; 7] = [
    Family::new(LootDefinitionKind::Table, "/definitions/lootTables"),
    Family::new(LootDefinitionKind::Archetype, "/definitions/lootArchetypes"),
    Family::new(
        LootDefinitionKind::PoolFilter,
        "/definitions/lootPoolFilters",
    ),
    Family::new(
        LootDefinitionKind::SecondaryChoiceSingleLayer,
        "/definitions/lootSecondaryChoiceSingleLayers",
    ),
    Family::new(
        LootDefinitionKind::SecondaryChoiceMultiLayer,
        "/definitions/lootSecondaryChoiceMultiLayers",
    ),
    Family::new(LootDefinitionKind::Theme, "/definitions/lootThemes"),
    Family::new(
        LootDefinitionKind::GlobalParams,
        "/definitions/lootGenerationGlobalParams",
    ),
];

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_loot_definitions_match_one_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_LOOT_DEFINITION_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Loot definition Catalog producer revision")?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &revision,
        |archive, measurement| {
            let session = equipment_input::full_source_session(
                archive,
                CatalogLanguageSelection::Selected(&[]),
            )?;
            let parsed = session.parsed(None)?;
            let mut origins = Origins::default();
            let mut tables = Vec::with_capacity(FAMILIES.len());
            for family in FAMILIES {
                tables.push(read_family(archive, &parsed, family, |snapshot| {
                    real_archive_catalog_table_parity::capture_rows(
                        snapshot.availability,
                        &snapshot.rows,
                        &snapshot.source_inventory,
                        &mut origins,
                        |row| {
                            row.pointer("/recordIndex/value")
                                .and_then(Value::as_u64)
                                .ok_or_else(|| {
                                    "expected an original Loot definition Record index".into()
                                })
                        },
                    )
                })?);
            }
            let tables: [Table; FAMILIES.len()] = tables
                .try_into()
                .map_err(|_| "expected every Loot definition family")?;
            let counts =
                measurement.verify_tables(FAMILIES.map(|family| family.path), &tables, &origins)?;
            real_archive_verification::print_line(&format!(
                "complete Loot definition SDK parity: rows={:?} sourceUses={} physicalSources={}",
                counts.rows, counts.source_uses, counts.physical_sources,
            ));
            Ok(())
        },
    )
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact dataset path"]
fn installed_loot_definition_cold_query() -> TestResult {
    let path = env::var("P4KX_LOOT_DEFINITION_SDK_COLD_DATASET")
        .map_err(|_| "configure one canonical Loot definition dataset path")?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path == path)
        .ok_or("expected one exact Loot definition dataset path")?;
    equipment_input::with_archive(|archive| {
        let session =
            equipment_input::full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
        let parsed = session.parsed(None)?;
        read_family(archive, &parsed, family, |snapshot| {
            let rows = snapshot.rows.len();
            let uses = snapshot.source_inventory.use_count();
            let sources = snapshot.source_inventory.source_count();
            if rows > 0 && (uses == 0 || sources == 0) {
                return Err("Loot definition rows have an empty source inventory".into());
            }
            real_archive_verification::print_line(&format!(
                "Loot definition SDK cold: dataset={path} rows={rows} inventoryUses={uses} inventorySources={sources}",
            ));
            Ok(())
        })
    })
}

fn read_family<T>(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    family: Family,
    inspect: impl FnOnce(LootDefinitionSnapshot<'_>) -> TestResult<T>,
) -> TestResult<T> {
    parsed.with_loot_definitions(family.kind, CatalogRecordSelection::All, None, |snapshot| {
        if snapshot.build.is_none() || snapshot.build.as_ref() != archive.build() {
            return Err(format!("{}: Loot definition Build does not match", family.path).into());
        }
        let rows = snapshot.rows.len();
        let coverage = &snapshot.coverage;
        require_complete(family.path, snapshot.availability, rows, coverage)?;
        real_archive_verification::print_line(&format!(
            "Loot definition SDK family: dataset={} availability={:?} rows={rows} skippedLeftovers={}",
            family.path, snapshot.availability, coverage.skipped_leftovers,
        ));
        inspect(snapshot)
    })?
}

fn require_complete(
    path: &str,
    availability: DatasetAvailability,
    rows: usize,
    coverage: &CatalogRecordCoverage,
) -> TestResult {
    let valid_disposition = match availability {
        DatasetAvailability::Available => true,
        DatasetAvailability::Absent => rows == 0,
        _ => false,
    };
    if !valid_disposition
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err(
            format!("{path}: expected complete Loot definition coverage and availability").into(),
        );
    }
    Ok(())
}

#[test]
fn loot_definition_parity_rejects_false_absence_and_incomplete_coverage() {
    let [table, ..] = FAMILIES;
    let path = table.path;
    let mut coverage = CatalogRecordCoverage::default();
    assert!(
        require_complete(path, DatasetAvailability::Available, 1, &coverage).is_ok(),
        "a populated declared table is available"
    );
    assert!(
        require_complete(path, DatasetAvailability::Available, 0, &coverage).is_ok(),
        "an empty declared table is available"
    );
    assert!(
        require_complete(path, DatasetAvailability::Absent, 0, &coverage).is_ok(),
        "the undeclared family has no rows"
    );
    assert!(
        require_complete(path, DatasetAvailability::Absent, 1, &coverage).is_err(),
        "a returned row contradicts absence"
    );
    assert!(
        require_complete(path, DatasetAvailability::Unavailable, 0, &coverage).is_err(),
        "an unavailable declaration cannot establish parity"
    );
    coverage.unreadable_headers.push(0);
    assert!(
        require_complete(path, DatasetAvailability::Absent, 0, &coverage).is_err(),
        "an unreadable header cannot establish absence"
    );
    coverage.unreadable_headers.clear();
    coverage.unreadable_bodies.push(0);
    assert!(
        require_complete(path, DatasetAvailability::Available, 1, &coverage).is_err(),
        "an unreadable body makes the returned rows incomplete"
    );
    coverage.unreadable_bodies.clear();
    coverage.incomplete_fields.push(0);
    assert!(
        require_complete(path, DatasetAvailability::Available, 1, &coverage).is_err(),
        "an incomplete field makes the returned rows incomplete"
    );
}
