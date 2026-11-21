// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::commodities::with_commodities;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::Archive;
use p4kx_testkit::{TestEntity, TestRecord};

use super::super::{COMMODITY_DEFINITIONS, COMMODITY_PRESENTATIONS};
use super::{Route, TestResult};

pub(super) fn fixture_record() -> TestRecord {
    TestEntity::new(
        8,
        "FixtureCommodity",
        "libs/foundry/records/entities/commodities/fixture.xml",
    )
    .record()
}

pub(super) fn read(archive: &Archive) -> TestResult<[Route; 2]> {
    const REMAINING: &str =
        "physical selection and synthetic catalog parity available; installed producer parity open";
    with_commodities(
        archive,
        CatalogRecordSelection::RecordIndex(7),
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| {
            Ok([
                Route::queried_rows(COMMODITY_DEFINITIONS, &snapshot.definitions, REMAINING)?,
                Route::queried_rows(COMMODITY_PRESENTATIONS, &snapshot.presentations, REMAINING)?,
            ])
        },
    )?
}
