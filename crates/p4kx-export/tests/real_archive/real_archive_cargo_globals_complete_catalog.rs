// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;

use super::{TestResult, record_index};
use crate::real_archive_catalog_table_parity::{Origins, Table, capture_table, verify_tables};
use crate::real_archive_verification::print_line;

pub(crate) const TABLE_PATHS: [&str; 2] = [
    "/configurations/cargoLoadingGlobals",
    "/configurations/shopCommodityGlobals",
];

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_cargo_sdk_matches_the_same_complete_catalog() -> TestResult {
    let counts = verify_tables(TABLE_PATHS, read_sdk)?;
    let [loading, prices] = counts.rows;
    print_line(&format!(
        "cargo SDK parity: loadingRows={loading} shopRows={prices} sourceUses={} physicalSources={}",
        counts.source_uses, counts.physical_sources,
    ));
    Ok(())
}

pub(crate) fn read_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<[Table; 2]> {
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let loading =
        parsed.with_cargo_loading_globals(CatalogRecordSelection::All, None, |sdk| {
            capture_table(&sdk, archive, origins, record_index)
        })??;
    let prices =
        parsed.with_shop_commodity_globals(CatalogRecordSelection::All, None, |sdk| {
            capture_table(&sdk, archive, origins, record_index)
        })??;
    Ok([loading, prices])
}
