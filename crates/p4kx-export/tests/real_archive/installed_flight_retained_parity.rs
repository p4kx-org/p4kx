// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries,
};
use p4kx_extraction::Archive;

use super::{SHIP_RECORD, TABLE, record_index, report_scoped_rows, selected_rows};
use crate::real_archive_catalog_table_parity::{
    Origins, Table, TestResult, with_bound_measurement_from_producer,
};
use crate::real_archive_vehicle_armor::parity as vehicle_armor;
use crate::real_archive_verification::print_line;

const PRODUCER_REVISION: &str = "P4KX_FLIGHT_CATALOG_PRODUCER_REVISION";

#[test]
#[ignore = "same Build: configure the installed Archive, completed Normal Catalog and physical Ship"]
fn installed_flight_matches_retained_complete_catalog() -> TestResult {
    let revision = std::env::var(PRODUCER_REVISION)?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        let mut origins = Origins::default();
        let table = capture_ship(&parsed, archive, &mut origins)?;
        measurement.verify_tables([TABLE], std::array::from_ref(&table), &origins)?;
        report_scoped_rows("Installed flight", "retained-parity", table.rows())
    })
}

#[test]
#[ignore = "same Build: configure the installed Archive, completed Normal Catalog and physical Ship"]
fn installed_flight_and_vehicle_armor_match_retained_complete_catalog() -> TestResult {
    let revision = std::env::var(PRODUCER_REVISION)?;
    let counts = with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        let mut origins = Origins::default();
        let flight = capture_ship(&parsed, archive, &mut origins)?;
        let armor = vehicle_armor::capture_all(&parsed, archive, &mut origins)?;
        measurement.verify_tables([TABLE, vehicle_armor::DATASET], &[flight, armor], &origins)
    })?;
    let [flight, armor] = counts.rows;
    print_line(&format!(
        "Installed flight and Vehicle Armor SDK parity: flight={flight} armor={armor} sourceUses={} physicalSources={}",
        counts.source_uses, counts.physical_sources,
    ));
    Ok(())
}

fn capture_ship(
    parsed: &ParsedCatalogQueries<'_>,
    archive: &Archive,
    origins: &mut Origins,
) -> TestResult<Table> {
    let index = record_index(SHIP_RECORD)?;
    let snapshot = parsed
        .read_installed_configurations(index, None)
        .map_err(|_| "read the nominated SDK installed flight scope")?;
    let rows = selected_rows(archive, index, &snapshot)?;
    origins.extend(&snapshot.source_inventory, &rows)?;
    Table::physical_ship(snapshot.availability, index, rows)
}
