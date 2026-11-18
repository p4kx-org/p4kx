// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::{BlueprintRewardSelection, BlueprintRewardSnapshot, reward_read};
use crate::CatalogError;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};

/// Supplies original reward occurrences in an explicit source-owner or inverse target scope.
/// Publish consumer-owned changes only after outer success and coverage checks.
///
/// # Errors
/// Returns original input, source-evidence, Archive-change or cancellation errors.
pub fn with_reward_sources<T>(
    archive: &Archive,
    selection: BlueprintRewardSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(BlueprintRewardSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?;
    session.with_blueprint_reward_sources(selection, cancel, inspect)
}

/// Supplies the same original reward survey from a decoded Entry of this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_reward_sources_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: BlueprintRewardSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(BlueprintRewardSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let session = CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?;
    session.with_blueprint_reward_sources(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies original reward occurrences without opening locale payloads.
    ///
    /// # Errors
    /// Returns query, source-evidence, Archive-change or cancellation errors.
    pub fn with_blueprint_reward_sources<T>(
        &self,
        selection: BlueprintRewardSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(BlueprintRewardSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_blueprint_reward_sources(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies the selected reward survey while reusing this parsed original DataForge.
    ///
    /// # Errors
    /// Returns query, source-evidence, Archive-change or cancellation errors.
    pub fn with_blueprint_reward_sources<T>(
        &self,
        selection: BlueprintRewardSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(BlueprintRewardSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            reward_read::with_parsed(archive, entry, forge, selection, cancel, inspect)
        })
    }
}
