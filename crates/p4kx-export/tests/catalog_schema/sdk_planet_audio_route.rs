// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::planet_audio::with_procedural_planet_audio;
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{Route, TestResult};

#[expect(
    dead_code,
    reason = "the schema witness uses the complete integration fixture"
)]
#[path = "../it/catalog_procedural_planet_audio_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("planet_audio_fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            r"Data\Game2.dcb",
            fixture::database(fixture::Layout::Complete),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    with_procedural_planet_audio(&archive, CatalogRecordSelection::All, None, |snapshot| {
        Route::queried_rows(
            "/definitions/proceduralPlanetAudio",
            snapshot.rows(),
            "synthetic producer parity witnessed; same-build installed producer parity open",
        )
    })?
}
