// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{Route, TestResult};

#[path = "sdk_hacking_fixture.rs"]
mod fixture;

const CHIPS: &str = "/configurations/hackingChips";
const DEVICES: &str = "/configurations/hackableDevices";

pub(super) fn read() -> TestResult<[Route; 2]> {
    const REMAINING: &str = "typed source queries and synthetic producer parity witnessed; same-build installed producer parity open";
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("hacking_fixture.p4k");
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
    Ok([
        parsed.with_hacking_chips(
            CatalogRecordSelection::RecordIndex(fixture::CHIP_RECORD),
            None,
            |snapshot| Route::queried_rows(CHIPS, snapshot.rows(), REMAINING),
        )??,
        parsed.with_hackable_devices(
            CatalogRecordSelection::RecordIndex(fixture::DEVICE_RECORD),
            None,
            |snapshot| Route::queried_rows(DEVICES, snapshot.rows(), REMAINING),
        )??,
    ])
}
