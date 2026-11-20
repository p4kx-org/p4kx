// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective canonical Commodity definitions and their presentations.

#[path = "commodity_query_read.rs"]
mod read;
#[path = "commodity_query_snapshot.rs"]
mod snapshot;

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

pub use crate::commodity_authoring_contract::{
    CommodityAuthoring, CommodityContainerLink, CommodityDisplay, CommodityPurchasable,
};
pub use crate::commodity_contract::{CommodityDefinition, CommodityPresentation};
pub use snapshot::CommoditySnapshot;

/// Supplies selected Commodity definitions and presentations from this Archive.
///
/// # Errors
/// Returns input, Archive change, source evidence or cancellation errors.
pub fn with_commodities<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CommoditySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_commodities(selection, cancel, inspect)
}

/// Supplies selected Commodity definitions and presentations from this decoded Entry.
///
/// # Errors
/// Returns input, Archive change, source origin, source evidence or cancellation errors.
pub fn with_commodities_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CommoditySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_commodities(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected Commodity definitions and presentations using the session's languages.
    ///
    /// # Errors
    /// Returns input, Archive change, source evidence or cancellation errors.
    pub fn with_commodities<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(CommoditySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_commodities(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected Commodity definitions and presentations, reusing this DataForge.
    ///
    /// # Errors
    /// Returns input, Archive change, source evidence or cancellation errors.
    pub fn with_commodities<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(CommoditySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::Query {
                archive,
                entry,
                forge,
                selection,
                requested_languages: self.languages(),
                cancel,
            }
            .run(inspect)
        })
    }
}
