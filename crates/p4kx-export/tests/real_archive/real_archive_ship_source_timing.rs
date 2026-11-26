// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::time::Instant;

use p4kx_export::Language;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};

use crate::real_archive_catalog_table_parity::TestResult;
use crate::real_archive_scoped_equipment_sdk::equipment_input;
use crate::real_archive_verification;

#[test]
#[ignore = "installed Archive: configure the exact Build, Ship source row census and locales"]
fn installed_ship_sources_marker_timing() -> TestResult {
    let expected = env::var("P4KX_SHIP_SOURCE_SDK_ROWS")?.parse::<usize>()?;
    let languages = env::var("P4KX_SHIP_SOURCE_SDK_LANGUAGES")?
        .split(',')
        .filter(|code| !code.is_empty())
        .map(str::parse::<Language>)
        .collect::<Result<Vec<_>, _>>()?;
    let opened = Instant::now();
    equipment_input::with_archive(|archive| {
        let session = CatalogQuerySession::open(
            archive,
            CatalogLanguageSelection::Selected(&languages),
            None,
        )?;
        let parsed = session.parsed(None)?;
        let started = Instant::now();
        let count = parsed.with_ship_sources(CatalogRecordSelection::All, None, |snapshot| {
            if expected == 0
                || snapshot.rows().len() != expected
                || snapshot.build() != archive.build()
                || !matches!(snapshot.availability(), DatasetAvailability::Available)
            {
                return Err("the Ship source timing probe differs from the nominated census");
            }
            Ok(snapshot.rows().len())
        })??;
        real_archive_verification::print_line(&format!(
            "Ship source Marker timing: rows={count} initialMillis={} queryMillis={}",
            started.duration_since(opened).as_millis(),
            started.elapsed().as_millis(),
        ));
        Ok(())
    })
}
