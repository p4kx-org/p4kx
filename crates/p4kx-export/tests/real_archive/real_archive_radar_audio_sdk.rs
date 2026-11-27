// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::radar_audio::{
    RadarAudioSelection, RadarAudioSnapshot, read_radar_audio_with_provenance_mode,
};
use p4kx_export::catalog_queries::{DatasetAvailability, ProvenanceMode};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
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

#[test]
#[ignore = "installed Archive: set P4KX_DATA_P4K and P4KX_RADAR_AUDIO_SDK_BUILD"]
fn installed_radar_audio_cold_query() -> TestResult {
    let expected = env::var("P4KX_RADAR_AUDIO_SDK_BUILD")
        .map_err(|_| "configure the nominated Radar Audio Build")?;
    let path = real_archive_path().map_err(|_| "configure the installed Archive path")?;
    let key = real_archive_key().map_err(|_| "configure the installed Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the nominated Archive")?;
    if archive
        .build()
        .is_none_or(|build| build.change_number() != expected)
        || expected.is_empty()
        || !expected.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("the installed Archive differs from the nominated Build".into());
    }
    let snapshot = read_complete(&archive)?;
    print_line(&format!(
        "Radar Audio SDK cold query: rows={} sourceUses={} physicalSources={}",
        snapshot.rows.len(),
        snapshot.source_inventory.iter_uses().len(),
        snapshot.source_inventory.iter_sources().len(),
    ));
    Ok(())
}

pub(crate) fn read_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<[Table; 1]> {
    let sdk = read_complete(archive)?;
    let table = capture_rows(
        sdk.availability,
        &sdk.rows,
        &sdk.source_inventory,
        origins,
        entry_index,
    )?;
    Ok([table])
}

fn read_complete(archive: &Archive) -> TestResult<RadarAudioSnapshot> {
    let sdk = read_radar_audio_with_provenance_mode(
        archive,
        RadarAudioSelection::All,
        ProvenanceMode::Full,
        None,
    )
    .map_err(|_| "read the nominated Archive's Radar Audio SDK scope")?;
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

    Ok(sdk)
}

fn entry_index(row: &Value) -> TestResult<u64> {
    row.pointer("/identity/entryIndex")
        .and_then(Value::as_u64)
        .ok_or_else(|| "required physical Radar Audio Entry index".into())
}
