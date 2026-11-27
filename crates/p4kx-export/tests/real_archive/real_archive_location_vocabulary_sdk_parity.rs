// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::location_vocabulary::{
    LocationVocabularyFamily as Family, LocationVocabularyRows, LocationVocabularySnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability, ParsedCatalogQueries,
};
use p4kx_extraction::Archive;
use serde::Serialize;
use serde_json::Value;

use super::equipment_input::{full_source_session, with_archive};
use crate::real_archive_catalog_table_parity::{
    Origins, Table, TestResult, capture_rows, with_bound_measurement_from_producer,
};
use crate::real_archive_verification::print_line;

const FAMILIES: [(Family, &str); 7] = [
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

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_location_vocabulary_matches_one_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_LOCATION_VOCABULARY_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact location vocabulary Catalog producer revision")?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let session = full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
        let parsed = session.parsed(None)?;
        let mut origins = Origins::default();
        let tables: [Table; FAMILIES.len()] = FAMILIES
            .into_iter()
            .map(|family| read_family(archive, &parsed, family, &mut origins))
            .collect::<TestResult<Vec<_>>>()?
            .try_into()
            .map_err(|_| "expected every location vocabulary family")?;
        let paths = FAMILIES.map(|(_, path)| path);
        let counts = measurement.verify_tables(paths, &tables, &origins)?;
        print_line(&format!(
            "complete location vocabulary SDK parity: rows={:?} sourceUses={} physicalSources={}",
            counts.rows, counts.source_uses, counts.physical_sources,
        ));
        Ok(())
    })
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact dataset path"]
fn installed_location_vocabulary_cold_query() -> TestResult {
    let selected_path = env::var("P4KX_LOCATION_VOCABULARY_SDK_COLD_DATASET")
        .map_err(|_| "configure one canonical location vocabulary dataset path")?;
    let selected = FAMILIES
        .into_iter()
        .find(|(_, path)| *path == selected_path)
        .ok_or("expected one exact location vocabulary dataset path")?;
    with_archive(|archive| {
        let session = full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
        let parsed = session.parsed(None)?;
        let mut origins = Origins::default();
        let table = read_family(archive, &parsed, selected, &mut origins)?;
        let (source_uses, physical_sources) = origins.contributing_counts();
        print_line(&format!(
            "location vocabulary SDK cold: dataset={} rows={} sourceUses={source_uses} physicalSources={physical_sources}",
            selected.1,
            table.rows().len(),
        ));
        Ok(())
    })
}

fn read_family(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    (family, path): (Family, &str),
    origins: &mut Origins,
) -> TestResult<Table> {
    parsed.with_location_vocabulary(family, CatalogRecordSelection::All, None, |snapshot| {
        let coverage = &snapshot.coverage;
        if snapshot.build.is_none()
            || snapshot.build.as_ref() != archive.build()
            || !coverage.unreadable_headers.is_empty()
            || !coverage.unreadable_bodies.is_empty()
            || !coverage.incomplete_fields.is_empty()
            || !matches!(
                snapshot.availability,
                DatasetAvailability::Available | DatasetAvailability::Absent
            )
        {
            return Err(format!("{path}: expected complete original coverage of the matching Build").into());
        }
        let table = match &snapshot.rows {
            LocationVocabularyRows::Classes(rows) => capture_family_rows(&snapshot, rows, origins),
            LocationVocabularyRows::Functions(rows) | LocationVocabularyRows::Sizes(rows) => {
                capture_family_rows(&snapshot, rows, origins)
            }
            LocationVocabularyRows::ResourceSlots(rows) => {
                capture_family_rows(&snapshot, rows, origins)
            }
            LocationVocabularyRows::ClassRegistries(rows)
            | LocationVocabularyRows::FunctionRegistries(rows)
            | LocationVocabularyRows::SizeRegistries(rows) => {
                capture_family_rows(&snapshot, rows, origins)
            }
            _ => Err(format!("{path}: unrecognized typed location vocabulary family").into()),
        }?;
        print_line(&format!(
            "location vocabulary SDK family: dataset={path} availability={:?} rows={} skippedLeftovers={}",
            snapshot.availability,
            table.rows().len(),
            coverage.skipped_leftovers,
        ));
        Ok(table)
    })?
}

fn capture_family_rows<Row: Serialize>(
    snapshot: &LocationVocabularySnapshot<'_>,
    rows: &[Row],
    origins: &mut Origins,
) -> TestResult<Table> {
    capture_rows(
        snapshot.availability,
        rows,
        &snapshot.source_inventory,
        origins,
        |row| {
            row.pointer("/recordIndex/value")
                .and_then(Value::as_u64)
                .ok_or_else(|| "expected an original location vocabulary Record index".into())
        },
    )
}
