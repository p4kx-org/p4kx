// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::location_vocabulary::with_contract_scopes;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{Route, TestResult};

#[expect(
    dead_code,
    reason = "the schema witness uses the complete integration fixture"
)]
#[path = "../it/catalog_contract_scope_sdk_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("contract_scope_fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            r"Data\Game2.dcb",
            fixture::database(fixture::Layout::Complete).build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    with_contract_scopes(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            Route::queried_rows(
                "/definitions/contractScopes",
                snapshot.rows(),
                "synthetic producer parity witnessed; same-build installed producer parity open",
            )
        },
    )?
}
