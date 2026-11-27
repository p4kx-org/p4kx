// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::super::{
    GLOBAL_JUMP_POINT_CONFIGURATIONS, JUMP_POINT_CONFIGURATIONS, JUMP_TUNNEL_FORCES,
};
use super::{Route, TestResult};

#[path = "sdk_jump_configuration_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<[Route; 3]> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("jump_configuration_fixture.p4k");
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
        parsed.with_jump_point_configurations(all, None, |snapshot| {
            Route::queried_rows(JUMP_POINT_CONFIGURATIONS, snapshot.rows(), None)
        })??,
        parsed.with_global_jump_point_configurations(all, None, |snapshot| {
            Route::queried_rows(GLOBAL_JUMP_POINT_CONFIGURATIONS, snapshot.rows(), None)
        })??,
        parsed.with_jump_tunnel_forces(all, None, |snapshot| {
            Route::queried_rows(JUMP_TUNNEL_FORCES, snapshot.rows(), None)
        })??,
    ])
}
