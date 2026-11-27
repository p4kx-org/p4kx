// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{CatalogRecordSelection, tags};
use p4kx_extraction::Archive;

use super::super::{ENTITY_TAGS, TAG_DEFINITIONS, TAG_PATHS, TAG_ROOTS};
use super::{Route, TestResult};

pub(super) fn entities(archive: &Archive) -> TestResult<Route> {
    tags::with_entity_tags(archive, CatalogRecordSelection::All, None, |snapshot| {
        Route::queried_rows(
            ENTITY_TAGS,
            &snapshot.rows,
            "synthetic source and Catalog parity available; installed producer parity open",
        )
    })?
}

pub(super) fn declarations(archive: &Archive) -> TestResult<[Route; 2]> {
    tags::with_tag_declarations(
        archive,
        tags::TagDeclarationFamily::All,
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            let definitions = snapshot.definitions.ok_or("Tag definitions")?;
            let roots = snapshot.roots.ok_or("TagDatabase root arrays")?;
            Ok([
                Route::queried_rows(TAG_DEFINITIONS, &definitions.rows, None)?,
                Route::queried_rows(TAG_ROOTS, &roots.rows, None)?,
            ])
        },
    )?
}

pub(super) fn paths(archive: &Archive) -> TestResult<Route> {
    tags::with_tag_paths(archive, CatalogRecordSelection::All, None, |snapshot| {
        Route::queried_rows(TAG_PATHS, &snapshot.rows, None)
    })?
}
