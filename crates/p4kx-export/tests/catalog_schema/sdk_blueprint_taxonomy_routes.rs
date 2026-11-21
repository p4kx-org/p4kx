// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::super::{BLUEPRINT_CATEGORIES, BLUEPRINT_CATEGORY_LISTS, GAMEPLAY_PROPERTIES};
use super::{Route, TestResult};

#[path = "../it/category_lists_v3_source_fixture.rs"]
mod category_lists;
#[path = "../it/gameplay_v3_schema.rs"]
#[expect(
    dead_code,
    reason = "The integration suite uses the alternate transformation variants"
)]
mod gameplay_v3_schema;
#[path = "../it/gameplay_v3_values.rs"]
#[expect(
    dead_code,
    reason = "The integration suite uses the alternate property values"
)]
mod gameplay_v3_values;

const REMAINING: &str =
    "typed SDK row and synthetic producer parity available; installed producer parity open";

pub(super) fn read() -> TestResult<[Route; 3]> {
    let (_directory, archive) = archive()?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let selection = CatalogRecordSelection::All;
    let lists = session.read_blueprint_category_lists(selection, None)?;
    let categories = session.read_blueprint_categories(selection, None)?;
    let properties = session.read_gameplay_properties(selection, None)?;
    Ok([
        Route::queried_rows(BLUEPRINT_CATEGORY_LISTS, &lists.rows, REMAINING)?,
        Route::queried_rows(BLUEPRINT_CATEGORIES, &categories.rows, REMAINING)?,
        Route::queried_rows(GAMEPLAY_PROPERTIES, &properties.rows, REMAINING)?,
    ])
}

fn archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("sdk_blueprint_taxonomy.p4k");
    let source = category_lists::with_clean_lists(gameplay_v3_values::simple()).build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", source).encrypted())
        .entry(
            TestEntry::stored(
                Language::English.global_ini_path(),
                b"resource_name=Resource English\nown_type=Own type\n",
            )
            .encrypted(),
        )
        .write_to(&path)?;
    Ok((
        directory,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}
