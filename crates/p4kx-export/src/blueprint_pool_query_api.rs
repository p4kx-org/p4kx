// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::{BlueprintPoolSnapshot, pool_read};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

/// Supplies original Blueprint-pool Records without localization or Blueprint dependencies.
/// Publish consumer-owned changes only after outer success and coverage checks.
///
/// # Errors
/// Returns original input, source-evidence, Archive-change or cancellation errors.
pub fn with_pools<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(BlueprintPoolSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?;
    session.with_blueprint_pools(selection, cancel, inspect)
}

/// Supplies the same original Blueprint pools from a decoded Entry of this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_pools_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(BlueprintPoolSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let session = CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?;
    session.with_blueprint_pools(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected original Blueprint pools without opening locale payloads.
    ///
    /// # Errors
    /// Returns query, source-evidence, Archive-change or cancellation errors.
    pub fn with_blueprint_pools<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(BlueprintPoolSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_blueprint_pools(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected Blueprint pools while reusing this parsed original DataForge.
    ///
    /// # Errors
    /// Returns query, source-evidence, Archive-change or cancellation errors.
    pub fn with_blueprint_pools<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(BlueprintPoolSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            pool_read::with_parsed(archive, entry, forge, selection, cancel, None, inspect)
        })
    }
}
