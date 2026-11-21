// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{Route, TestResult};

#[path = "sdk_item_durability_fixture.rs"]
mod fixture;

pub(super) const fn datasets() -> [&'static str; 3] {
    [
        fixture::DURABILITY,
        fixture::DEGRADATION,
        fixture::DISTORTION,
    ]
}

pub(super) fn read() -> TestResult<[Route; 3]> {
    const REMAINING: &str = "typed Item query and synthetic producer parity witnessed; complete installed producer parity open";
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("durability_fixture.p4k");
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
    let selection = CatalogRecordSelection::All;
    Ok([
        parsed.with_durability(selection, None, |snapshot| {
            Route::queried_rows(fixture::DURABILITY, snapshot.rows(), REMAINING)
        })??,
        parsed.with_degradation(selection, None, |snapshot| {
            Route::queried_rows(fixture::DEGRADATION, snapshot.rows(), REMAINING)
        })??,
        parsed.with_distortion(selection, None, |snapshot| {
            Route::queried_rows(fixture::DISTORTION, snapshot.rows(), REMAINING)
        })??,
    ])
}
