// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::{collections::HashSet, env};

use p4kx_export::catalog_queries::equipment::{
    FacilityInstance, FacilityInteriorCoverageSnapshot, FacilityInteriorPlacementSnapshot,
    FacilityInteriorPlacementVisitSummary,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, DatasetAvailability,
};
use p4kx_extraction::{Archive, Build, DataForgeEntry, SourceValueState};
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    Origins, StreamedInteriorDigests, Table, TestResult, wire_row,
    with_bound_measurement_from_producer,
};
use crate::real_archive_scoped_equipment_sdk::equipment_input::{
    full_source_session, with_archive,
};
use crate::real_archive_verification::print_line;

const DATASET: &str = "/placements/facilityInteriorCoverage";
const PRODUCER: &str = "P4KX_FACILITY_INTERIOR_CATALOG_PRODUCER_REVISION";
const SHIP: &str = "P4KX_FACILITY_INTERIOR_SDK_RECORD";

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_all_facility_interior_placements_match_retained_complete_catalog() -> TestResult {
    let revision = env::var(PRODUCER)?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let scratch = tempfile::tempdir()?;
        let mut evidence = StreamedInteriorDigests::new(scratch.path());
        let session = full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
        let summary = visit_installed(&session, nominated_build(archive)?, |owner| {
            evidence.retain(owner)
        })?;
        let counts = evidence.compare(measurement)?;
        if counts.rows != [summary.coverage_count, summary.placement_count] {
            return Err("retained Interior row counts differ from the completed visitor".into());
        }
        print_line(&format!(
            "complete Facility Interior SDK parity: sourceUses={} physicalSources={}",
            counts.source_uses, counts.physical_sources,
        ));
        Ok(())
    })
}

#[test]
#[ignore = "standalone cold Interior placement visit: configure the exact installed Build"]
fn installed_all_facility_interior_placements_cold_visit() -> TestResult {
    with_archive(|archive| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
        visit_installed(&session, nominated_build(archive)?, |_| Ok(()))?;
        Ok(())
    })
}

fn visit_installed(
    session: &CatalogQuerySession<'_>,
    build: &Build,
    mut inspect: impl FnMut(&FacilityInteriorPlacementSnapshot) -> TestResult,
) -> TestResult<FacilityInteriorPlacementVisitSummary> {
    let mut seen = HashSet::new();
    let mut incomplete = 0;
    let summary =
        session
            .parsed(None)?
            .visit_facility_interior_placements(None, |owner| -> TestResult {
                let coverage = &owner.coverage;
                if coverage.build.as_ref() != Some(build)
                    || coverage.root_state != SourceValueState::Present
                    || !seen.insert(coverage.ship_record_index)
                {
                    return Err("the installed visit did not emit a distinct physical Ship".into());
                }
                incomplete += usize::from(!coverage.complete);
                inspect(owner)
            })?;
    if summary.build.as_ref() != Some(build)
        || !matches!(summary.availability, DatasetAvailability::Available)
        || summary.owner_count != seen.len()
        || summary.incomplete_owner_count != incomplete
        || summary.placement_count == 0
        || summary.coverage_count == 0
    {
        return Err("the installed Build has no verified nonempty Interior placement visit".into());
    }
    print_line(&format!(
        "Facility Interior SDK visit: ships={} coverage={} placements={} incompleteShips={incomplete}",
        summary.owner_count, summary.coverage_count, summary.placement_count,
    ));
    Ok(summary)
}

#[test]
#[ignore = "standalone nomination: configure the installed Build and private exact Record path"]
fn installed_facility_interior_nominate_record() -> TestResult {
    let path = env::var("P4KX_FACILITY_INTERIOR_SDK_RECORD_PATH")?;
    with_archive(|archive| {
        let input = DataForgeEntry::read(archive)?;
        let forge = input.open()?;
        let mut records = forge.records_at_path(&path);
        let record = records
            .next()
            .ok_or("the requested Record path is absent")?;
        if records.next().is_some() {
            return Err("the requested Record path has multiple physical occurrences".into());
        }
        print_line(&format!(
            "Facility Interior SDK Record index: {}",
            record.index()
        ));
        Ok(())
    })
}

#[test]
#[ignore = "same Build: configure the installed Archive, completed Normal Catalog and physical Ship"]
fn installed_facility_interior_matches_retained_complete_catalog() -> TestResult {
    let revision = env::var(PRODUCER)?;
    let ship: u32 = env::var(SHIP)?.parse()?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let (rows, origins) = selected_rooms(archive, ship)?;
        measurement.verify_interior_ship_rows(DATASET, ship, &rows, None, &origins)?;
        report("retained-parity", &rows, &origins);
        Ok(())
    })
}

#[test]
#[ignore = "same Build: configure the installed Archive, completed Normal Catalog and physical Ship"]
fn installed_facility_interior_placements_match_retained_complete_catalog() -> TestResult {
    let revision = env::var(PRODUCER)?;
    let ship: u32 = env::var(SHIP)?.parse()?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let selected = selected_placements(archive, ship)?;
        measurement.verify_interior_ship_rows(
            DATASET,
            ship,
            &selected.coverage,
            Some(&selected.placements),
            &selected.origins,
        )?;
        selected.report("retained-placements");
        Ok(())
    })
}

#[test]
#[ignore = "standalone cold placement query: configure the installed Build and physical Ship"]
fn installed_facility_interior_placements_cold_query() -> TestResult {
    let ship: u32 = env::var(SHIP)?.parse()?;
    with_archive(|archive| {
        selected_placements(archive, ship)?.report("cold-placements");
        Ok(())
    })
}

#[test]
#[ignore = "standalone cold query: configure the installed Build and physical Ship"]
fn installed_facility_interior_cold_query() -> TestResult {
    let ship: u32 = env::var(SHIP)?.parse()?;
    with_archive(|archive| {
        let (rows, origins) = selected_rooms(archive, ship)?;
        report("cold-query", &rows, &origins);
        Ok(())
    })
}

fn selected_rooms(archive: &Archive, ship: u32) -> TestResult<(Vec<Value>, Origins)> {
    let build = nominated_build(archive)?;
    let session = full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
    let snapshot = session
        .parsed(None)?
        .read_facility_interior_coverage(ship, None)?;
    checked_rooms(build, ship, &snapshot)
}

struct SelectedInterior {
    coverage: Vec<Value>,
    placements: Table,
    origins: Origins,
}

impl SelectedInterior {
    fn report(&self, lane: &str) {
        let (uses, sources) = self.origins.contributing_counts();
        print_line(&format!(
            "Facility Interior SDK {lane}: coverage={} placements={} sourceUses={uses} physicalSources={sources}",
            self.coverage.len(),
            self.placements.rows().len(),
        ));
    }
}

fn selected_placements(archive: &Archive, ship: u32) -> TestResult<SelectedInterior> {
    let build = nominated_build(archive)?;
    let session = full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
    let snapshot = session
        .parsed(None)?
        .read_facility_interior_placements(ship, None)?;
    let (coverage, mut origins) = checked_rooms(build, ship, &snapshot.coverage)?;
    let rows = physical_placements(ship, &snapshot.rows)?;
    origins.extend(&snapshot.coverage.source_inventory, &rows)?;
    let placements = Table::interior_placements(snapshot.availability, ship, rows)?;
    Ok(SelectedInterior {
        coverage,
        placements,
        origins,
    })
}

fn nominated_build(archive: &Archive) -> TestResult<&Build> {
    let build = archive
        .build()
        .ok_or("the installed Archive lacks a Build")?;
    if build.change_number() != env::var("P4KX_NEW_SDK_BUILD")? {
        return Err("the installed Archive differs from the nominated Build".into());
    }
    Ok(build)
}

fn checked_rooms(
    build: &Build,
    ship: u32,
    snapshot: &FacilityInteriorCoverageSnapshot,
) -> TestResult<(Vec<Value>, Origins)> {
    if snapshot.build.as_ref() != Some(build)
        || snapshot.ship_record_index != ship
        || snapshot.root_state != SourceValueState::Present
        || !snapshot.complete
    {
        return Err(
            "the nominated Ship has no complete SDK room scope on the matching Build".into(),
        );
    }
    let rows = snapshot
        .rows
        .iter()
        .map(wire_row)
        .collect::<TestResult<Vec<_>>>()?;
    if !rows.iter().any(|row| {
        row.pointer("/containers/0/container/state")
            .and_then(Value::as_str)
            == Some("present")
    }) {
        return Err("the selected Ship must open at least one original room".into());
    }
    let mut origins = Origins::default();
    origins.extend(&snapshot.source_inventory, &rows)?;
    let (uses, sources) = origins.contributing_counts();
    if uses == 0 || sources == 0 {
        return Err("the selected rooms must retain contributing physical source evidence".into());
    }
    Ok((rows, origins))
}

fn physical_placements(ship: u32, rows: &[FacilityInstance]) -> TestResult<Vec<Value>> {
    if rows.is_empty() {
        return Err(
            "the selected Ship must contain an original Interior facility placement".into(),
        );
    }
    if !rows.iter().all(|row| {
        row.ship_record_index().value() == Some(&ship)
            && row.interior_placement().is_some()
            && row.configurations().next().is_some()
    }) {
        return Err(
            "every SDK placement must belong to the nominated Ship and retain a configuration"
                .into(),
        );
    }
    rows.iter().map(wire_row).collect()
}

fn report(lane: &str, rows: &[Value], origins: &Origins) {
    let (uses, sources) = origins.contributing_counts();
    print_line(&format!(
        "Facility Interior SDK {lane}: rows={} sourceUses={uses} physicalSources={sources}",
        rows.len()
    ));
}
