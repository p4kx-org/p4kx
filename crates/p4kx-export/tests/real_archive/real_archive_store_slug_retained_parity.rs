// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::with_ship_store_slugs;
use p4kx_extraction::Archive;
use serde_json::Value;

use super::{STORE, report_scoped_rows, store_selection};
use crate::real_archive_catalog_table_parity::{
    TestResult, wire_row, with_bound_measurement_from_producer,
};
use crate::real_archive_mining_sdk_parity::verify_mining_and_row_arrays;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_store_slugs_match_retained_complete_catalog() -> TestResult {
    let revision = std::env::var("P4KX_STORE_SLUG_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Store Catalog producer revision")?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let rows = store_rows(archive)?;
        measurement.verify_row_array(STORE, &rows)?;
        report_scoped_rows("Store slug", "retained-parity", &rows)
    })
}

#[test]
#[ignore = "same Build: configure the installed Archive and the exact completed Normal Catalog"]
fn installed_mining_and_store_slugs_match_retained_complete_catalog() -> TestResult {
    let revision = std::env::var("P4KX_STORE_SLUG_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Store Catalog producer revision")?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let rows = store_rows(archive)?;
        verify_mining_and_row_arrays(measurement, archive, &[(STORE, &rows)])?;
        report_scoped_rows("Store slug", "retained-parity", &rows)
    })
}

fn store_rows(archive: &Archive) -> TestResult<Vec<Value>> {
    with_ship_store_slugs(archive, None, None, |snapshot| {
        store_selection(archive, &snapshot, None)?;
        let rows = snapshot.rows();
        if !rows
            .iter()
            .zip(rows.iter().skip(1))
            .all(|(left, right)| left.slug() < right.slug())
        {
            return Err("the complete Store array must have a strict slug order".into());
        }
        rows.iter().map(wire_row).collect::<TestResult<Vec<_>>>()
    })
    .map_err(|_| "read the unfiltered same-Build SDK Store rows")?
}
