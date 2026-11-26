// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::crafting::{CrafterHostSnapshot, CrafterSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries, ProvenanceMode,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, Filter, RecordTree, SourceValueState, Value,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value as Json;

use crate::real_archive_catalog_table_parity::{Origins, TestResult, wire_row};
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification::print_line;

#[path = "real_archive_crafter_nomination.rs"]
mod nomination;

#[path = "real_archive_crafter_original.rs"]
mod original;

const STATIONS: &str = "/definitions/crafters";
const HOSTS: &str = "/projections/crafterHosts";
const BUILD: &str = "P4KX_CRAFTER_WITNESS_BUILD";
const STATION_INDEX: &str = "P4KX_CRAFTER_WITNESS_STATION_INDEX";
const SHIP_INDEX: &str = "P4KX_CRAFTER_WITNESS_SHIP_INDEX";
const HOST_FILTER: &str = "P4KX_CRAFTER_WITNESS_FILTER";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_crafter_station_matches_original_sdk_and_literal_producer() -> TestResult {
    let archive = open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the original DataForge Entry")?;
    let forge = entry.open().map_err(|_| "parse original DataForge")?;
    let station = original::station(&forge, &watch, optional_index(STATION_INDEX)?)?;
    let path = station.record_path();
    if path.is_empty() || path.contains(['*', '?']) {
        return Err("the original station has no usable literal producer path".into());
    }
    let filter = Filter::new(path);
    original::exact_filter(
        &forge,
        &watch,
        &filter,
        &BTreeSet::from([station.record_index()]),
    )?;
    let catalog = produce(&archive, &entry, &filter)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_provenance_mode(ProvenanceMode::Full);
    let parsed = session.parsed(None)?;
    let mut origins = Origins::default();
    let row = compare_station(&archive, &parsed, &station, &mut origins)?;
    compare_table(&catalog, STATIONS, &[row])?;
    origins.verify_in_document(&catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during witness")?;
    print_line(
        "Installed Crafter witness: one original station and its scoped SDK row matched the literal filtered producer and all cited origins",
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_crafter_hosts_match_original_sdk_and_exact_filtered_producer() -> TestResult {
    let archive = open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the original DataForge Entry")?;
    let forge = entry.open().map_err(|_| "parse original DataForge")?;
    let ship =
        optional_index(SHIP_INDEX)?.ok_or("configure the private physical host Record index")?;
    let mounts = original::resolve_mounts(&archive, &forge, &watch, ship)?;
    let filter_text =
        std::env::var(HOST_FILTER).map_err(|_| "configure the private exact producer filter")?;
    let filter = Filter::new(&filter_text);
    let expected = mounts.keys().copied().chain([ship]).collect();
    original::exact_filter(&forge, &watch, &filter, &expected)?;
    let catalog = produce(&archive, &entry, &filter)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_provenance_mode(ProvenanceMode::Full);
    let parsed = session.parsed(None)?;
    let snapshot = parsed.read_crafter_hosts(ship, None)?;
    let mut origins = Origins::default();
    let host_rows = verified_host_rows(&archive, ship, &snapshot, &mounts, &mut origins)?;
    compare_table(&catalog, HOSTS, &host_rows)?;
    verify_host_stations(&archive, &forge, &parsed, &catalog, &expected, &mut origins)?;
    origins.verify_in_document(&catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during witness")?;
    print_line(&format!(
        "Installed Crafter host witness: {} physical mount groups and {} original routes matched both filtered tables and their cited origins",
        snapshot.rows.len(),
        mounts.values().map(Vec::len).sum::<usize>(),
    ));
    Ok(())
}

fn verify_host_stations(
    archive: &Archive,
    forge: &p4kx_extraction::DataForge<'_>,
    parsed: &ParsedCatalogQueries<'_>,
    catalog: &Json,
    selection: &BTreeSet<u32>,
    origins: &mut Origins,
) -> TestResult {
    let mut scoped = Vec::new();
    let watch = ArchiveWatch::new(archive);
    for &index in selection {
        let result = parsed.with_crafters(
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| {
                if snapshot.rows.is_empty() && complete(&snapshot.coverage) {
                    return Ok(None);
                }
                let station = original::station(forge, &watch, Some(index))?;
                capture_station(archive, &station, &snapshot, origins).map(Some)
            },
        )??;
        scoped.extend(result);
    }
    scoped.sort_by_cached_key(|row| {
        (
            row.pointer("/guid/value")
                .and_then(Json::as_str)
                .map(str::to_owned),
            row.pointer("/recordIndex/value").and_then(Json::as_u64),
        )
    });
    compare_table(catalog, STATIONS, &scoped)
}

fn compare_station(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    station: &RecordTree<'_>,
    origins: &mut Origins,
) -> TestResult<Json> {
    parsed.with_crafters(
        CatalogRecordSelection::RecordIndex(station.record_index()),
        None,
        |snapshot| capture_station(archive, station, &snapshot, origins),
    )?
}

fn capture_station(
    archive: &Archive,
    station: &RecordTree<'_>,
    snapshot: &CrafterSnapshot<'_>,
    origins: &mut Origins,
) -> TestResult<Json> {
    if snapshot.build.as_ref() != archive.build()
        || !matches!(snapshot.availability, DatasetAvailability::Available)
        || !complete(&snapshot.coverage)
    {
        return Err(
            "the exact station SDK scope is incomplete or differs from the original Build".into(),
        );
    }
    let [row] = snapshot.rows.as_slice() else {
        return Err("the exact station scope did not return one physical row".into());
    };
    if row.source().record_index().value() != Some(&station.record_index()) {
        return Err("the exact row addresses a different physical station".into());
    }
    let (ordinal, source) = original::component(station)?;
    let typed = row
        .component()
        .value()
        .ok_or("the selected original crafter component")?;
    if typed.component_ordinal().value() != Some(&ordinal) {
        return Err("the SDK selected another physical component".into());
    }
    if let Some(Value::Array(original)) = source.field("queues")
        && typed
            .queues()
            .value()
            .is_none_or(|slots| slots.len() != original.len())
    {
        return Err("original queue slots differ from the independent source".into());
    }
    let wire = wire_row(row)?;
    origins.extend(&snapshot.source_inventory, std::slice::from_ref(&wire))?;
    Ok(wire)
}

fn verified_host_rows(
    archive: &Archive,
    index: u32,
    snapshot: &CrafterHostSnapshot,
    expected: &original::Mounts,
    origins: &mut Origins,
) -> TestResult<Vec<Json>> {
    verify_mounts(archive, index, snapshot, expected)?;
    let rows = snapshot
        .rows
        .iter()
        .map(wire_row)
        .collect::<TestResult<Vec<_>>>()?;
    origins.extend(&snapshot.source_inventory, &rows)?;
    Ok(rows)
}

fn verify_mounts(
    archive: &Archive,
    index: u32,
    snapshot: &CrafterHostSnapshot,
    expected: &original::Mounts,
) -> TestResult {
    if snapshot.build.as_ref() != archive.build()
        || !matches!(snapshot.availability, DatasetAvailability::Available)
        || snapshot.host_state != SourceValueState::Present
        || !complete(&snapshot.loadout_coverage)
        || !complete(&snapshot.crafter_coverage)
        || !snapshot.dependencies.is_complete()
        || snapshot.rows.len() != expected.len()
    {
        return Err(
            "the exact host SDK scope has incomplete or different original input coverage".into(),
        );
    }
    for (row, (target, expected_routes)) in snapshot.rows.iter().zip(expected) {
        if row.ship_record_index().value() != Some(&index)
            || row.crafter().target_record_index().value() != Some(target)
        {
            return Err("the SDK did not retain the exact physical host and crafter".into());
        }
        let routes = row
            .mount_positions()
            .iter()
            .map(|slot| {
                slot.value()
                    .filter(|position| position.loadout_record_index() == index)
                    .map(|position| position.port_ordinals().to_vec())
                    .ok_or("an original mount position is unavailable")
            })
            .collect::<Result<Vec<_>, _>>()?;
        if &routes != expected_routes {
            return Err("the original mount routes differ in count, order or position".into());
        }
    }
    Ok(())
}

const fn complete(coverage: &CatalogRecordCoverage) -> bool {
    coverage.unreadable_headers.is_empty()
        && coverage.unreadable_bodies.is_empty()
        && coverage.incomplete_fields.is_empty()
}

fn produce(archive: &Archive, entry: &DataForgeEntry, filter: &Filter) -> TestResult<Json> {
    let options = CatalogOptions {
        filter: Some(filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, options.with_full_provenance())
        .map_err(|_| "build the exact filtered witness Catalog")?;
    let catalog: Json = serde_json::from_slice(built.json())
        .map_err(|_| "parse the exact filtered witness Catalog")?;
    verify_build(archive, &catalog)?;
    Ok(catalog)
}

fn compare_table(catalog: &Json, dataset: &str, expected: &[Json]) -> TestResult {
    let table = catalog
        .pointer(dataset)
        .ok_or("the filtered canonical table is missing")?;
    if table.get("availability").and_then(Json::as_str) != Some("available")
        || table
            .get("rows")
            .and_then(Json::as_array)
            .is_none_or(|rows| rows.as_slice() != expected)
    {
        return Err(format!("the exact SDK and filtered producer differ at {dataset}").into());
    }
    Ok(())
}

fn optional_index(variable: &str) -> TestResult<Option<u32>> {
    match std::env::var(variable) {
        Ok(value) => value
            .parse()
            .map(Some)
            .map_err(|_| "invalid private physical Record index".into()),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => Err("unreadable private physical Record index".into()),
    }
}

fn open_archive() -> TestResult<Archive> {
    let path = real_archive_path().map_err(|_| "configure the original installed Archive")?;
    let key = real_archive_key().map_err(|_| "configure the installed Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the configured Archive")?;
    let expected =
        std::env::var(BUILD).map_err(|_| "configure the expected installed Build changelist")?;
    let build = archive
        .build()
        .ok_or("the installed Archive needs a complete Build identity")?;
    if expected.is_empty()
        || !expected.bytes().all(|byte| byte.is_ascii_digit())
        || build.change_number() != expected
        || [
            build.branch(),
            build.version(),
            build.change_number(),
            build.build_id(),
        ]
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err("the configured Archive has a different or incomplete Build identity".into());
    }
    Ok(archive)
}
