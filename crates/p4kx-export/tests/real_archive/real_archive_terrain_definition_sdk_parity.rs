// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::terrain::LooseDefinitionSelection;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, DatasetAvailability, LooseDefinitionSnapshot, ParsedCatalogQueries,
    ProvenanceMode, climate, creatures, terrain,
};
use p4kx_extraction::Archive;
use serde::Serialize;
use serde_json::Value;

use super::equipment_input;
use crate::real_archive_catalog_table_parity::{self, Origins, Table, TestResult};
use crate::real_archive_verification;

#[derive(Clone, Copy)]
enum Family {
    Ecosystems,
    ShadingClasses,
    GeologySets,
    GeologyTextureSets,
    GeologyAssets,
    Creatures,
    ClimateAssets,
}

const ARCHIVE_ONLY: [Family; 4] = [
    Family::Ecosystems,
    Family::ShadingClasses,
    Family::GeologySets,
    Family::GeologyTextureSets,
];
const RECORD_LINKS: [Family; 3] = [
    Family::GeologyAssets,
    Family::Creatures,
    Family::ClimateAssets,
];
const TABLE_COUNT: usize = ARCHIVE_ONLY.len() + RECORD_LINKS.len();

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::Ecosystems => "/definitions/ecosystems",
            Self::ShadingClasses => "/definitions/shadingClasses",
            Self::GeologySets => "/definitions/geologySets",
            Self::GeologyTextureSets => "/definitions/geologyTextureSets",
            Self::GeologyAssets => "/definitions/geologyAssets",
            Self::Creatures => "/definitions/creatures",
            Self::ClimateAssets => "/definitions/climateAssets",
        }
    }
}

enum Evidence<'a> {
    Cold,
    Retained(&'a mut Origins),
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_terrain_definitions_match_one_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_TERRAIN_DEFINITION_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact terrain definition Catalog producer revision")?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &revision,
        |archive, measurement| {
            let mut origins = Origins::default();
            let mut tables = Vec::with_capacity(TABLE_COUNT);
            retain_tables(archive, None, ARCHIVE_ONLY, &mut origins, &mut tables)?;
            {
                let session = equipment_input::full_source_session(
                    archive,
                    CatalogLanguageSelection::Selected(&[]),
                )?;
                let parsed = session.parsed(None)?;
                retain_tables(
                    archive,
                    Some(&parsed),
                    RECORD_LINKS,
                    &mut origins,
                    &mut tables,
                )?;
            }
            let tables: [Table; TABLE_COUNT] = tables
                .try_into()
                .map_err(|_| "expected all seven terrain definition tables")?;
            let paths: [&str; TABLE_COUNT] = ARCHIVE_ONLY
                .into_iter()
                .chain(RECORD_LINKS)
                .map(Family::path)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "expected seven terrain definition dataset paths")?;
            let compared = measurement.verify_tables(paths, &tables, &origins)?;
            real_archive_verification::print_line(&format!(
                "complete terrain definition SDK parity: rows={:?} sourceUses={} physicalSources={}",
                compared.rows, compared.source_uses, compared.physical_sources,
            ));
            Ok(())
        },
    )
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact dataset path"]
fn installed_terrain_definition_cold_query() -> TestResult {
    let path = env::var("P4KX_TERRAIN_DEFINITION_SDK_COLD_DATASET")
        .map_err(|_| "configure one canonical terrain definition dataset path")?;
    let family = ARCHIVE_ONLY
        .into_iter()
        .chain(RECORD_LINKS)
        .find(|family| family.path() == path)
        .ok_or("expected one exact terrain definition dataset path")?;
    equipment_input::with_archive(|archive| {
        read_family(archive, None, family, Evidence::Cold)?;
        Ok(())
    })
}

fn retain_tables(
    archive: &Archive,
    parsed: Option<&ParsedCatalogQueries<'_>>,
    families: impl IntoIterator<Item = Family>,
    origins: &mut Origins,
    tables: &mut Vec<Table>,
) -> TestResult {
    for family in families {
        let table = read_family(archive, parsed, family, Evidence::Retained(origins))?
            .ok_or("expected a retained terrain definition table")?;
        tables.push(table);
    }
    Ok(())
}

fn read_family(
    archive: &Archive,
    parsed: Option<&ParsedCatalogQueries<'_>>,
    family: Family,
    evidence: Evidence<'_>,
) -> TestResult<Option<Table>> {
    let all = LooseDefinitionSelection::All;
    let mode = ProvenanceMode::Full;
    match family {
        Family::Ecosystems => observe(
            archive,
            family,
            &terrain::read_ecosystems_with_provenance_mode(archive, all, mode, None)?,
            evidence,
        ),
        Family::ShadingClasses => observe(
            archive,
            family,
            &terrain::read_shading_classes_with_provenance_mode(archive, all, mode, None)?,
            evidence,
        ),
        Family::GeologySets => observe(
            archive,
            family,
            &terrain::read_geology_sets_with_provenance_mode(archive, all, mode, None)?,
            evidence,
        ),
        Family::GeologyTextureSets => observe(
            archive,
            family,
            &terrain::read_geology_texture_sets_with_provenance_mode(archive, all, mode, None)?,
            evidence,
        ),
        Family::GeologyAssets => {
            let sdk = match parsed {
                Some(queries) => queries.read_geology_assets(all, None)?,
                None => {
                    terrain::read_geology_assets_with_provenance_mode(archive, all, mode, None)?
                }
            };
            observe(archive, family, &sdk, evidence)
        }
        Family::Creatures => {
            let sdk = match parsed {
                Some(queries) => queries.read_creatures(all, None)?,
                None => creatures::read_creatures_with_provenance_mode(archive, all, mode, None)?,
            };
            observe(archive, family, &sdk, evidence)
        }
        Family::ClimateAssets => {
            let sdk = match parsed {
                Some(queries) => queries.read_climate_assets(all, None)?,
                None => {
                    climate::read_climate_assets_with_provenance_mode(archive, all, mode, None)?
                }
            };
            observe(archive, family, &sdk, evidence)
        }
    }
}

fn observe<Row: Serialize>(
    archive: &Archive,
    family: Family,
    sdk: &LooseDefinitionSnapshot<Row>,
    evidence: Evidence<'_>,
) -> TestResult<Option<Table>> {
    let path = family.path();
    if sdk.build.is_none() || sdk.build.as_ref() != archive.build() {
        return Err(format!("{path} requires the exact installed Build").into());
    }
    require_scope(
        path,
        sdk.availability,
        sdk.rows.len(),
        sdk.is_selection_complete(),
        sdk.entries.failed_dependencies,
    )?;
    real_archive_verification::print_line(&format!(
        "terrain definition SDK family: dataset={path} availability={:?} rows={} intentionallySkipped={}",
        sdk.availability,
        sdk.rows.len(),
        sdk.entries.intentionally_skipped,
    ));
    match evidence {
        Evidence::Retained(origins) => real_archive_catalog_table_parity::capture_rows(
            sdk.availability,
            &sdk.rows,
            &sdk.source_inventory,
            origins,
            physical_entry,
        )
        .map(Some),
        Evidence::Cold => {
            let uses = sdk.source_inventory.use_count();
            let sources = sdk.source_inventory.source_count();
            if !sdk.rows.is_empty() && (uses == 0 || sources == 0) {
                return Err(format!("{path} rows have an empty source inventory").into());
            }
            real_archive_verification::print_line(&format!(
                "terrain definition SDK cold: dataset={path} rows={} inventoryUses={uses} inventorySources={sources}",
                sdk.rows.len(),
            ));
            Ok(None)
        }
    }
}

fn require_scope(
    path: &str,
    availability: DatasetAvailability,
    rows: usize,
    selection_complete: bool,
    failed_dependencies: usize,
) -> TestResult {
    let known_disposition = match availability {
        DatasetAvailability::Available => true,
        DatasetAvailability::Absent => rows == 0,
        _ => false,
    };
    if !known_disposition || !selection_complete || failed_dependencies != 0 {
        return Err(format!(
            "{path} has incomplete original owner coverage (selectionComplete={selection_complete} failedDependencies={failed_dependencies})"
        ).into());
    }
    Ok(())
}

fn physical_entry(row: &Value) -> TestResult<u32> {
    row.pointer("/identity/entryIndex")
        .and_then(Value::as_u64)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| "expected a physical terrain definition Entry index".into())
}

#[test]
fn terrain_definition_parity_distinguishes_physical_entry_aliases() -> TestResult {
    use serde_json::json;

    let first = json!({"identity":{"entryIndex":7,"sourcePath":{"value":"shared"}},"guid":{"value":"same"}});
    let second = json!({"identity":{"entryIndex":8,"sourcePath":{"value":"shared"}},"guid":{"value":"same"}});
    assert_ne!(
        physical_entry(&first)?,
        physical_entry(&second)?,
        "GUID and path aliases retain separate Entries"
    );
    assert!(
        physical_entry(&json!({"entryIndex":7,"guid":{"value":"same"}})).is_err(),
        "the physical identity must be owned by the terrain definition",
    );
    Ok(())
}

#[test]
fn terrain_definition_parity_rejects_unknown_or_incomplete_owner_scope() -> TestResult {
    let path = Family::ShadingClasses.path();
    require_scope(path, DatasetAvailability::Absent, 0, true, 0)?;
    require_scope(path, DatasetAvailability::Available, 0, true, 0)?;
    assert!(
        require_scope(path, DatasetAvailability::Absent, 1, true, 0).is_err(),
        "an observed Entry contradicts absence"
    );
    assert!(
        require_scope(path, DatasetAvailability::Unavailable, 0, true, 0).is_err(),
        "an unavailable family cannot establish an empty table"
    );
    assert!(
        require_scope(path, DatasetAvailability::Available, 1, false, 0).is_err(),
        "an undecidable owner scope may omit rows"
    );
    assert!(
        require_scope(path, DatasetAvailability::Available, 1, true, 1).is_err(),
        "a failed required dependency prevents complete parity"
    );
    Ok(())
}
