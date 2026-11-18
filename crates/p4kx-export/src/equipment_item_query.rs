// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::snapshot::ItemDefinitionSnapshot;
use super::{read, rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

/// Supplies selected original Item definitions with their authored presentation and physical fields.
/// Publish consumer-owned changes only after success and coverage checks.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_item_definitions<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Definitions, T>(archive, None, selection, languages, cancel, inspect)
}

/// Supplies the same canonical rows from an already decoded Entry of this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_item_definitions_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Definitions, T>(
        archive,
        Some(entry),
        selection,
        languages,
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies selected original Item definitions with their authored presentation and physical fields through this session.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_item_definitions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_item_definitions(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected original Item definitions with their authored presentation and physical fields through this parsed view.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_item_definitions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<rows::Definitions, T>(
                archive,
                entry,
                forge,
                selection,
                self.languages(),
                cancel,
                inspect,
            )
        })
    }
}
