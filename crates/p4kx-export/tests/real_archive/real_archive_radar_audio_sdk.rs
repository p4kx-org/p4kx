// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::radar_audio::{RadarAudioSelection, read_radar_audio};
use p4kx_extraction::Archive;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    Origins, Table, TestResult, capture_rows, verify_tables,
};
use crate::real_archive_verification::print_line;

pub(crate) const TABLES: [&str; 1] = ["/configurations/radarAudio"];

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_radar_audio_sdk_matches_the_same_complete_catalog() -> TestResult {
    let counts = verify_tables(TABLES, read_sdk)?;
    let [radar] = counts.rows;
    print_line(&format!(
        "radar audio SDK parity: rows={radar} sourceUses={} physicalSources={}",
        counts.source_uses, counts.physical_sources,
    ));
    Ok(())
}

pub(crate) fn read_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<[Table; 1]> {
    let sdk = read_radar_audio(archive, RadarAudioSelection::All, None)?;
    let rows = sdk.rows.len();
    let entries = sdk.entries;
    if sdk.build.as_ref() != archive.build()
        || !matches!(sdk.availability, DatasetAvailability::Available)
        || !sdk.directory.source_set_complete()
        || rows == 0
        || entries.selected != rows
        || entries.read_successful != rows
        || entries.projected != rows
        || entries.intentionally_skipped != 0
        || entries.failed_required_reads != 0
        || entries.failed_projections != 0
        || entries.dependencies != 0
        || entries.failed_dependencies != 0
        || entries.failed_selection_candidates != 0
    {
        return Err("the Radar Audio SDK must read every selected physical configuration".into());
    }

    let table = capture_rows(
        sdk.availability,
        &sdk.rows,
        &sdk.source_inventory,
        origins,
        entry_index,
    )?;
    Ok([table])
}

fn entry_index(row: &Value) -> TestResult<u64> {
    row.pointer("/identity/entryIndex")
        .and_then(Value::as_u64)
        .ok_or_else(|| "required physical Radar Audio Entry index".into())
}
