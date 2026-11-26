// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::Archive;

use crate::real_archive_catalog_table_parity::{
    Origins, Table, TestResult, capture_table, with_bound_measurement_from_producer,
};
use crate::real_archive_resource_configurations_sdk::component_identity;
use crate::real_archive_scoped_equipment_sdk::equipment_input::full_source_session;
use crate::real_archive_verification::print_line;

const TABLES: [&str; 5] = [
    "/configurations/fuelTanks",
    "/configurations/fuelIntakes",
    "/configurations/resourceContainers",
    "/configurations/resourceComponents",
    "/configurations/resourceConnections",
];

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K, P4KX_CATALOG_MEASUREMENT_DIR and P4KX_RESOURCE_SUPPLY_PRODUCER_REVISION"]
fn installed_resource_supply_sdk_matches_one_complete_catalog() -> TestResult {
    let revision = std::env::var("P4KX_RESOURCE_SUPPLY_PRODUCER_REVISION")
        .map_err(|_| "configure the exact resource-supply Catalog producer revision")?;
    let counts = with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let mut origins = Origins::default();
        let tables = read_sdk(archive, &mut origins)?;
        measurement.verify_tables(TABLES, &tables, &origins)
    })?;
    let [tanks, intakes, containers, components, connections] = counts.rows;
    print_line(&format!(
        "Resource supply SDK parity: fuelTankRows={tanks} fuelIntakeRows={intakes} resourceContainerRows={containers} resourceComponentRows={components} resourceConnectionRows={connections} sourceUses={} physicalSources={}",
        counts.source_uses, counts.physical_sources,
    ));
    Ok(())
}

fn read_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<[Table; 5]> {
    let session = full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
    let parsed = session.parsed(None)?;
    let all = CatalogRecordSelection::All;
    Ok([
        parsed.with_fuel_tanks(all, None, |snapshot| {
            capture_table(&snapshot, archive, origins, component_identity)
        })??,
        parsed.with_fuel_intakes(all, None, |snapshot| {
            capture_table(&snapshot, archive, origins, component_identity)
        })??,
        parsed.with_resource_containers(all, None, |snapshot| {
            capture_table(&snapshot, archive, origins, component_identity)
        })??,
        parsed.with_resource_components(all, None, |snapshot| {
            capture_table(&snapshot, archive, origins, component_identity)
        })??,
        parsed.with_resource_connections(all, None, |snapshot| {
            capture_table(&snapshot, archive, origins, component_identity)
        })??,
    ])
}
