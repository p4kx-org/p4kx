// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, ParsedCatalogQueries,
};
use p4kx_extraction::Archive;

use crate::real_archive_catalog_table_parity::{
    Origins, Table, TestResult, capture_table, with_bound_measurement_from_producer,
};
use crate::real_archive_resource_configurations_sdk::component_identity;
use crate::real_archive_scoped_equipment_sdk::equipment_input::full_source_session;
use crate::real_archive_verification::print_line;

pub(crate) const DATASET: &str = "/configurations/vehicleArmor";

#[test]
#[ignore = "same Build: configure the installed Archive and the exact completed Normal Catalog"]
fn installed_vehicle_armor_matches_retained_complete_catalog() -> TestResult {
    let revision = std::env::var("P4KX_VEHICLE_ARMOR_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Vehicle Armor Catalog producer revision")?;
    let counts = with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let session = full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
        let parsed = session.parsed(None)?;
        let mut origins = Origins::default();
        let table = capture_all(&parsed, archive, &mut origins)?;
        measurement.verify_tables([DATASET], &[table], &origins)
    })?;
    let [rows] = counts.rows;
    print_line(&format!(
        "Vehicle Armor SDK parity: rows={rows} sourceUses={} physicalSources={}",
        counts.source_uses, counts.physical_sources,
    ));
    Ok(())
}

pub(crate) fn capture_all(
    parsed: &ParsedCatalogQueries<'_>,
    archive: &Archive,
    origins: &mut Origins,
) -> TestResult<Table> {
    parsed.with_vehicle_armor(CatalogRecordSelection::All, None, |snapshot| {
        capture_table(&snapshot, archive, origins, component_identity)
    })?
}
