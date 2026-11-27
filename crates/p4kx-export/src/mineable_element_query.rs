// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::snapshot::MineableElementSnapshot;
use super::{read, rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

/// Supplies selected canonical mineable elements without building world placements.
/// Publish caller-owned changes only after success and examination of owner and dependency coverage.
///
/// # Errors
/// Returns input, source-evidence or cancellation errors.
pub fn with_mineable_elements<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MineableElementSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Elements, T>(
        archive,
        None,
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

/// Supplies selected canonical rows from a decoded Entry of the same Archive.
///
/// # Errors
/// Returns input, source-origin, source-evidence or cancellation errors.
pub fn with_mineable_elements_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MineableElementSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Elements, T>(
        archive,
        Some(entry),
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies this session's selected canonical rows without a world walk.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_mineable_elements<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MineableElementSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_mineable_elements(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected rows while retaining this view's DataForge indexes.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_mineable_elements<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MineableElementSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<rows::Elements, T>(
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
