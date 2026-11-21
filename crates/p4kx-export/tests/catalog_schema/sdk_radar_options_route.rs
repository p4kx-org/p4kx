// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{Route, TestResult};

#[expect(
    dead_code,
    reason = "the SDK witness uses the valid case of the shared integration fixture"
)]
#[path = "../it/catalog_resource_network_fixture.rs"]
mod fixture;

const NETWORK_REMAINING: &str = "typed source query and synthetic producer parity witnessed; complete installed producer parity open";
const RADAR_REMAINING: &str =
    "scoped installed producer parity witnessed; complete Catalog parity remains open";

pub(super) fn read() -> TestResult<[Route; 2]> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("radar_options_fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            r"Data\Game2.dcb",
            fixture::database(fixture::Layout::Complete).build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let selection = CatalogRecordSelection::All;
    Ok([
        parsed.with_radar_options(selection, None, |snapshot| {
            Route::queried_rows(
                "/configurations/radarOptions",
                snapshot.rows(),
                RADAR_REMAINING,
            )
        })??,
        parsed.with_resource_network_parameters(selection, None, |snapshot| {
            Route::queried_rows(
                "/configurations/resourceNetworkParameters",
                snapshot.rows(),
                NETWORK_REMAINING,
            )
        })??,
    ])
}
