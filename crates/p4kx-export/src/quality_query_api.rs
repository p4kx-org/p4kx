// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::read::Query;
use super::rows::{Distributions, Quantizations};
use super::{CatalogRecordSelection, DistributionSnapshot, QuantizationSnapshot};
use crate::CatalogError;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};

/// Supplies complete selected distribution and location-override definitions without target reads.
/// Publish owned results after outer success and examination of source coverage.
///
/// # Errors
/// Returns input, source-evidence or cancellation errors.
pub fn with_distributions<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(DistributionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_quality_distributions(selection, cancel, inspect)
}

/// Supplies selected distribution definitions from a decoded Entry of the same Archive.
///
/// # Errors
/// Returns input, source-origin-mismatch, source-evidence or cancellation errors.
pub fn with_distributions_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(DistributionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_quality_distributions(selection, cancel, inspect)
}

/// Supplies complete selected quantization definitions without computing an inverse resource map.
/// Publish owned results after outer success and examination of source coverage.
///
/// # Errors
/// Returns input, source-evidence or cancellation errors.
pub fn with_quantizations<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(QuantizationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_quality_quantizations(selection, cancel, inspect)
}

/// Supplies selected quantization definitions from a decoded Entry of the same Archive.
///
/// # Errors
/// Returns input, source-origin-mismatch, source-evidence or cancellation errors.
pub fn with_quantizations_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(QuantizationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_quality_quantizations(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected canonical distribution and override declarations without locale reads.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors; inspect source coverage before publication.
    pub fn with_quality_distributions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(DistributionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_quality_distributions(selection, cancel, inspect)
    }

    /// Supplies selected canonical quantization declarations without inverse resource reads.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors; inspect source coverage before publication.
    pub fn with_quality_quantizations<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(QuantizationSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_quality_quantizations(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected distribution definitions using this view's DataForge indexes.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors; inspect source coverage before publication.
    pub fn with_quality_distributions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(DistributionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            Query {
                archive,
                entry,
                forge,
                selection,
                cancel,
                observer: None,
            }
            .run::<Distributions, T>(inspect)
        })
    }

    /// Supplies selected quantization definitions using this view's DataForge indexes.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors; inspect source coverage before publication.
    pub fn with_quality_quantizations<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(QuantizationSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            Query {
                archive,
                entry,
                forge,
                selection,
                cancel,
                observer: None,
            }
            .run::<Quantizations, T>(inspect)
        })
    }
}
