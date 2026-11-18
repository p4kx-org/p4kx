// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::snapshot::ItemBoundsSnapshot;
use super::{read, rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

/// Supplies selected original occupancy and bounds without localization or world discovery.
/// Publish consumer-owned changes only after success and coverage checks.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_item_bounds<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemBoundsSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Bounds, T>(
        archive,
        None,
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

/// Supplies the same canonical rows from an already decoded Entry of this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_item_bounds_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemBoundsSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Bounds, T>(
        archive,
        Some(entry),
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies selected original occupancy and bounds without localization or world discovery through this session.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_item_bounds<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemBoundsSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_item_bounds(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected original occupancy and bounds without localization or world discovery through this parsed view.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_item_bounds<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemBoundsSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<rows::Bounds, T>(
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
