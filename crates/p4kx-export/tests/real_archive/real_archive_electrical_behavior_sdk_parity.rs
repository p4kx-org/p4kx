// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::weather::{self, ElectricalBehaviorSnapshot};
use p4kx_export::catalog_queries::{
    CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_extraction::Archive;
use serde_json::Value;

use super::equipment_input;
use crate::real_archive_catalog_table_parity::{self, Origins, TestResult};
use crate::real_archive_verification;

const DATASET: &str = "/configurations/electricalBehaviors";

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_electrical_behaviors_match_one_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_WEATHER_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact weather Catalog producer revision")?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &revision,
        |archive, measurement| {
            let mut origins = Origins::default();
            let table = read_all(archive, |snapshot| {
                real_archive_catalog_table_parity::capture_rows(
                    snapshot.availability(),
                    snapshot.rows(),
                    snapshot.source_inventory(),
                    &mut origins,
                    wire_record_index,
                )
            })?;
            let counts = measurement.verify_tables([DATASET], &[table], &origins)?;
            real_archive_verification::print_line(&format!(
                "complete weather SDK parity: rows={:?} sourceUses={} physicalSources={}",
                counts.rows, counts.source_uses, counts.physical_sources,
            ));
            Ok(())
        },
    )
}

#[test]
#[ignore = "standalone cold query: configure the installed Archive and exact Build"]
fn installed_electrical_behaviors_cold_query() -> TestResult {
    equipment_input::with_archive(|archive| {
        read_all(archive, |snapshot| {
            let mut previous = None;
            for row in snapshot.rows() {
                let index = row
                    .source()
                    .record_index()
                    .value()
                    .copied()
                    .ok_or("missing the physical ElectricalBehavior Record index")?;
                if previous.is_some_and(|previous| previous >= index) {
                    return Err(
                        "ElectricalBehavior Records are not in unique physical order".into(),
                    );
                }
                previous = Some(index);
            }
            Ok(())
        })
    })
}

fn read_all<T>(
    archive: &Archive,
    inspect: impl FnOnce(&ElectricalBehaviorSnapshot<'_>) -> TestResult<T>,
) -> TestResult<T> {
    weather::with_electrical_behaviors(archive, CatalogRecordSelection::All, None, |snapshot| {
        if snapshot.build().is_none() || snapshot.build() != archive.build() {
            return Err("the weather query must report the matching installed Build".into());
        }
        let discovery = snapshot.definition_discovery();
        if !discovery.is_complete() {
            return Err(format!(
                "incomplete weather definition discovery: directoryComplete={} failedRecords={} failedEntries={} incompleteRecords={} incompleteFields={}",
                discovery.directory_complete(),
                discovery.failed_records().len(),
                discovery.failed_entries().len(),
                discovery.incomplete_records().len(),
                discovery.incomplete_fields(),
            )
            .into());
        }
        let availability = snapshot.availability();
        let rows = snapshot.rows().len();
        let coverage = snapshot.coverage();
        let evidence = row_evidence(availability, rows, coverage)?;
        let inventory = snapshot.source_inventory();
        let uses = inventory.use_count();
        let sources = inventory.source_count();
        if rows != 0 && (uses == 0 || sources == 0) {
            return Err("physical ElectricalBehavior Records lack original source evidence".into());
        }
        real_archive_verification::print_line(&format!(
            "weather SDK family: dataset={DATASET} availability={availability:?} rowEvidence={evidence} rows={rows} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
            coverage.skipped_leftovers,
        ));
        inspect(&snapshot)
    })?
}

fn row_evidence(
    availability: DatasetAvailability,
    rows: usize,
    coverage: &CatalogRecordCoverage,
) -> TestResult<&'static str> {
    if !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
        || coverage.skipped_leftovers != 0
    {
        return Err(format!("incomplete ElectricalBehavior Record coverage: {coverage:?}").into());
    }
    match (availability, rows) {
        (DatasetAvailability::Available, 0) => Ok("complete-empty"),
        (DatasetAvailability::Available, _) => Ok("present"),
        (DatasetAvailability::Absent, 0) => Ok("absent-type"),
        _ => Err("expected established ElectricalBehavior availability and complete rows".into()),
    }
}

fn wire_record_index(row: &Value) -> TestResult<u32> {
    row.pointer("/source/recordIndex/value")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| "missing the canonical weather physical Record key".into())
}

#[test]
fn weather_parity_requires_the_physical_record_and_known_complete_availability() -> TestResult {
    use serde_json::json;

    let physical = json!({"source": {"recordIndex": {"value": 5}}});
    let wrong_path = json!({"recordIndex": {"value": 5}});
    assert_eq!(wire_record_index(&physical)?, 5, "use the original Record");
    assert!(
        wire_record_index(&wrong_path).is_err(),
        "a top-level field cannot substitute for a physical Record"
    );

    let mut coverage = CatalogRecordCoverage::default();
    assert_eq!(
        row_evidence(DatasetAvailability::Available, 0, &coverage)?,
        "complete-empty",
        "a declared type may have no physical Records"
    );
    assert_eq!(
        row_evidence(DatasetAvailability::Absent, 0, &coverage)?,
        "absent-type",
        "absence is a distinct declaration result"
    );
    assert!(
        row_evidence(DatasetAvailability::Unavailable, 0, &coverage).is_err(),
        "an unresolved declaration cannot witness absence"
    );
    assert!(
        row_evidence(DatasetAvailability::Absent, 1, &coverage).is_err(),
        "absence cannot have physical Records"
    );
    coverage.unreadable_bodies.push(5);
    assert!(
        row_evidence(DatasetAvailability::Absent, 0, &coverage).is_err(),
        "a failed body cannot witness complete coverage"
    );
    Ok(())
}
