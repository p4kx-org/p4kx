// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::env;

use p4kx_export::catalog_queries::equipment::{
    LoadoutFacilityPlacementSnapshot, LoadoutFacilityPlacementVisitSummary,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, DatasetAvailability, ProvenanceMode,
};
use p4kx_extraction::Archive;

use crate::real_archive_catalog_table_parity::{
    Origins, Table, TestResult, wire_row, with_bound_measurement_from_producer,
};
use crate::real_archive_scoped_equipment_sdk::equipment_input::with_archive;
use crate::real_archive_verification::print_line;

const TABLE: &str = "/placements/facilityInstances";
const PRODUCER: &str = "P4KX_FACILITY_LOADOUT_CATALOG_PRODUCER_REVISION";

#[test]
#[ignore = "real Archive: run a fresh process with the exact installed Build"]
fn installed_all_loadout_facility_placements_cold_visit() -> TestResult {
    with_archive(|archive| {
        let summary = visit_installed(archive, ProvenanceMode::MarkersOnly, |_| Ok(()))?;
        report(&summary);
        Ok(())
    })
}

#[test]
#[ignore = "same Build: compare the ordered Loadout rows with the exact completed Normal producer"]
fn installed_all_loadout_facility_placements_match_retained_complete_catalog() -> TestResult {
    let revision = env::var(PRODUCER)?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let mut origins = Origins::default();
        let mut rows = Vec::new();
        let summary = visit_installed(archive, ProvenanceMode::Full, |owner| {
            let start = rows.len();
            for row in &owner.rows {
                rows.push(wire_row(row)?);
            }
            origins.extend(
                &owner.source_inventory,
                rows.get(start..).ok_or("retained owner rows")?,
            )
        })?;
        let table = Table::loadout_facilities(summary.availability, rows)?;
        let compared = measurement.verify_tables([TABLE], &[table], &origins)?;
        if compared.rows != [summary.placement_count] {
            return Err("retained Loadout placement count differs from the complete visit".into());
        }
        report(&summary);
        print_line(&format!(
            "Loadout facility retained parity: sourceUses={} physicalSources={}",
            compared.source_uses, compared.physical_sources,
        ));
        Ok(())
    })
}

fn visit_installed(
    archive: &Archive,
    mode: ProvenanceMode,
    mut inspect: impl FnMut(&LoadoutFacilityPlacementSnapshot) -> TestResult,
) -> TestResult<LoadoutFacilityPlacementVisitSummary> {
    let build = archive.build().ok_or("expected an installed Build")?;
    if build.change_number() != env::var("P4KX_NEW_SDK_BUILD")? {
        return Err("the selected installed Build differs from the nominated Build".into());
    }
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?
            .with_provenance_mode(mode);
    let mut owners = HashSet::new();
    let mut rows = 0;
    let mut incomplete = 0;
    let summary = session.visit_loadout_facility_placements(None, |owner| {
        if owner.build.as_ref() != Some(build) || !owners.insert(owner.ship_record_index) {
            return Err(
                "the visit did not deliver distinct physical owners from the exact Build".into(),
            );
        }
        if owner.rows.iter().any(|row| {
            row.ship_record_index().value() != Some(&owner.ship_record_index)
                || row.loadout_position().is_none()
        }) {
            return Err("the visit returned a placement outside the Loadout owner scope".into());
        }
        rows += owner.rows.len();
        incomplete += usize::from(!owner.is_complete());
        inspect(owner)
    })?;
    if summary.build.as_ref() != Some(build)
        || !matches!(summary.availability, DatasetAvailability::Available)
        || summary.owner_count != owners.len()
        || summary.placement_count != rows
        || summary.incomplete_owner_count != incomplete
        || rows == 0
    {
        return Err(
            "the exact installed Build must emit nonempty, accounted Loadout placements".into(),
        );
    }
    Ok(summary)
}

fn report(summary: &LoadoutFacilityPlacementVisitSummary) {
    print_line(&format!(
        "Loadout facility SDK visit: ships={} placements={} incompleteShips={}",
        summary.owner_count, summary.placement_count, summary.incomplete_owner_count,
    ));
}
