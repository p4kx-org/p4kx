// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::game_modes::with_radar_options;
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{Route, TestResult};

#[expect(
    dead_code,
    reason = "the SDK witness uses the valid case of the shared integration fixture"
)]
#[path = "../it/catalog_resource_network_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("radar_options_fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            r"Data\Game2.dcb",
            fixture::database(fixture::Layout::Complete).build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    with_radar_options(&archive, CatalogRecordSelection::All, None, |snapshot| {
        Route::queried_rows(
            "/configurations/radarOptions",
            snapshot.rows(),
            "typed source query and synthetic producer parity witnessed; same-build installed producer parity open",
        )
    })?
}
