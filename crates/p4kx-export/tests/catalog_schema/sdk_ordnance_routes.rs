// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::super::{EMP, EXPLOSION_DEFINITIONS, EXPLOSIVES, MISSILE_CONFIGURATIONS};
use super::{Route, TestResult};

#[path = "sdk_ordnance_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<[Route; 4]> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("ordnance_fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            r"Data\Game2.dcb",
            fixture::database().build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let all = CatalogRecordSelection::All;
    Ok([
        parsed.with_emp(all, None, |snapshot| {
            Route::queried_rows(EMP, snapshot.rows(), None)
        })??,
        parsed.with_explosives(all, None, |snapshot| {
            Route::queried_rows(EXPLOSIVES, snapshot.rows(), None)
        })??,
        parsed.with_missile_configurations(all, None, |snapshot| {
            Route::queried_rows(MISSILE_CONFIGURATIONS, snapshot.rows(), None)
        })??,
        parsed.with_explosion_definitions(all, None, |snapshot| {
            Route::queried_rows(EXPLOSION_DEFINITIONS, snapshot.rows(), None)
        })??,
    ])
}
