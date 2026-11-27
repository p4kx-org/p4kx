// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use p4kx_extraction::CanonicalReadObserver;
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::snapshot::ItemSourceSnapshot;
use super::{read, rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

/// Supplies selected Item header, component selections, class and playability.
/// Variant decisions require a separate candidate survey.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_item_sources<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemSourceSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::ItemSources, T>(
        archive,
        None,
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

/// Supplies the same Item source fields from a decoded Entry of this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_item_sources_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemSourceSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::ItemSources, T>(
        archive,
        Some(entry),
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies selected Item source fields while reusing this session.
    ///
    /// # Errors
    /// Returns query, source-evidence, Archive-change or cancellation errors.
    pub fn with_item_sources<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemSourceSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_item_sources(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected Item source fields while reusing this parsed DataForge.
    ///
    /// # Errors
    /// Returns query, source-evidence, Archive-change or cancellation errors.
    pub fn with_item_sources<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemSourceSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<rows::ItemSources, T>(
                archive,
                entry,
                forge,
                selection,
                &[],
                cancel,
                inspect,
            )
        })
    }
}

#[cfg(test)]
pub(crate) fn observe_item_source_query<T>(
    parsed: &ParsedCatalogQueries<'_>,
    selection: CatalogRecordSelection,
    observer: Arc<dyn CanonicalReadObserver>,
    inspect: impl FnOnce(ItemSourceSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    parsed.project(None, |archive, entry, forge| {
        read::Query {
            archive,
            entry,
            forge,
            selection,
            languages: &[],
            cancel: None,
        }
        .run::<rows::ItemSources, T>(Some(observer), inspect)
    })
}
