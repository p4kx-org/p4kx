// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::{CraftingGlobalSnapshot, read};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

/// Supplies original crafting-global Records without localization or Blueprint dependencies.
/// Publish consumer-owned changes only after outer success and coverage checks.
///
/// # Errors
/// Returns original input, source-evidence, Archive-change or cancellation errors.
pub fn with_global_params<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CraftingGlobalSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?;
    session.with_crafting_global_params(selection, cancel, inspect)
}

/// Supplies the same original crafting globals from a decoded Entry of this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_global_params_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CraftingGlobalSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let session = CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?;
    session.with_crafting_global_params(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected original crafting globals without opening locale payloads.
    ///
    /// # Errors
    /// Returns query, source-evidence, Archive-change or cancellation errors.
    pub fn with_crafting_global_params<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(CraftingGlobalSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_crafting_global_params(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected crafting globals while reusing this parsed original DataForge.
    ///
    /// # Errors
    /// Returns query, source-evidence, Archive-change or cancellation errors.
    pub fn with_crafting_global_params<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(CraftingGlobalSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed(archive, entry, forge, selection, cancel, inspect)
        })
    }
}
