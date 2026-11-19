// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    Origins, Table, TestResult, capture_table, verify_two_tables,
};
use crate::real_archive_verification::print_line;

const SHIELDS: &str = "/configurations/shieldGenerators";
const QUANTUM: &str = "/configurations/quantumDrives";

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_ship_system_sdk_matches_the_same_complete_catalog() -> TestResult {
    let counts = verify_two_tables([SHIELDS, QUANTUM], read_sdk)?;
    let [shields, quantum] = counts.rows;
    print_line(&format!(
        "ship system SDK parity: shieldRows={shields} quantumRows={quantum} sourceUses={} physicalSources={}",
        counts.source_uses, counts.physical_sources,
    ));
    Ok(())
}

fn read_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<[Table; 2]> {
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let shields = parsed.with_shield_generators(CatalogRecordSelection::All, None, |sdk| {
        capture_table(&sdk, archive, origins, component_identity)
    })??;
    let quantum = parsed.with_quantum_drives(CatalogRecordSelection::All, None, |sdk| {
        capture_table(&sdk, archive, origins, component_identity)
    })??;
    Ok([shields, quantum])
}

fn component_identity(row: &Value) -> TestResult<(u64, u64)> {
    let owner = row
        .pointer("/ownerRecordIndex/value")
        .and_then(Value::as_u64)
        .ok_or("required physical ship-system owner index")?;
    let ordinal = row
        .pointer("/componentOrdinal/value")
        .and_then(Value::as_u64)
        .ok_or("required original ship-system component ordinal")?;
    Ok((owner, ordinal))
}
