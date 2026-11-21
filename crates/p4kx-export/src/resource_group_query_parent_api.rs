// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::request::Query;
use super::snapshot::ResourceGroupParentSnapshot;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

/// Supplies selected parent choices through the canonical group-family survey.
///
/// Publish consumer changes after successful completion and coverage examination.
///
/// # Errors
/// Returns input, source-evidence or cancellation errors.
pub fn with_resource_group_parents<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ResourceGroupParentSnapshot) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_resource_group_parents(selection, cancel, inspect)
}

/// Supplies selected canonical rows from an Entry bound to the same Archive.
///
/// # Errors
/// Returns input, source-origin, source-evidence or cancellation errors.
pub fn with_resource_group_parents_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ResourceGroupParentSnapshot) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_resource_group_parents(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies this session's selected canonical parent choices with a complete canonical group-family survey.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_resource_group_parents<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ResourceGroupParentSnapshot) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_resource_group_parents(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected canonical parent choices with a complete canonical group-family survey through the retained `DataForge` view.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_resource_group_parents<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ResourceGroupParentSnapshot) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            Query {
                archive,
                entry,
                forge,
                selection,
                languages: &[],
                cancel,
            }
            .run_parents(inspect)
        })
    }
}
