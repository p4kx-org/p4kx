// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::read::Query;
use super::snapshot::BlueprintSnapshot;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

/// Supplies complete selected physical Blueprints, tiers and canonical presentation.
/// Publish owned results after outer success and examination of owner and dependency coverage.
///
/// # Errors
/// Returns input, source-evidence or cancellation errors.
pub fn with_blueprints<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(BlueprintSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_blueprints(selection, cancel, inspect)
}

/// Supplies selected physical Blueprints from an already decoded Entry of the same Archive.
///
/// # Errors
/// Returns input, origin-mismatch, source-evidence or cancellation errors.
pub fn with_blueprints_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(BlueprintSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_blueprints(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected complete Blueprint datasets using this session's locale policy.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors; inspect coverage before publication.
    pub fn with_blueprints<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(BlueprintSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_blueprints(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected complete Blueprint datasets while reusing this view's DataForge indexes.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors; inspect coverage before publication.
    pub fn with_blueprints<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(BlueprintSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            Query {
                archive,
                entry,
                forge,
                selection,
                languages: self.languages(),
                cancel,
                observer: None,
            }
            .run(inspect)
        })
    }
}
