// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::request::Query;
use super::snapshot::ResourceGroupSnapshot;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

/// Supplies selected canonical group definitions and ordered memberships without a complete Catalog.
/// Publish consumer changes only after success and examination of all relevant coverage.
///
/// # Errors
/// Returns input, source-evidence or cancellation errors.
pub fn with_resource_groups<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ResourceGroupSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_resource_groups(selection, cancel, inspect)
}

/// Supplies selected canonical rows from an Entry bound to the same Archive.
///
/// # Errors
/// Returns input, source-origin, source-evidence or cancellation errors.
pub fn with_resource_groups_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ResourceGroupSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_resource_groups(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies this session's selected canonical group definitions and ordered memberships.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_resource_groups<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ResourceGroupSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_resource_groups(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected canonical group definitions and ordered memberships through the retained `DataForge` view.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_resource_groups<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ResourceGroupSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            Query {
                archive,
                entry,
                forge,
                selection,
                languages: self.languages(),
                cancel,
            }
            .run_direct(inspect)
        })
    }
}
