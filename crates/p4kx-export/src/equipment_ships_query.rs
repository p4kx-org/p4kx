// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::read::Query;
use super::{ShipSource, ShipSourceSnapshot};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::variant_use_sets::VariantSourceUseSets;
use crate::{CatalogError, CatalogProvenanceView};

/// Selected canonical Ship rows with unfiltered variant decisions.
#[derive(Debug)]
#[must_use]
pub struct ShipSnapshot<'source> {
    sources: ShipSourceSnapshot<'source>,
    variant_sets: VariantSourceUseSets,
}

impl<'source> ShipSnapshot<'source> {
    pub(super) const fn new(
        sources: ShipSourceSnapshot<'source>,
        variant_sets: VariantSourceUseSets,
    ) -> Self {
        Self {
            sources,
            variant_sets,
        }
    }

    /// Borrows the canonical selected Ship rows.
    #[must_use]
    pub fn rows(&self) -> &[ShipSource<'source>] {
        self.sources.rows()
    }

    /// Borrows original Ship fields, text resolution and coverage for this request.
    #[must_use]
    pub const fn sources(&self) -> &ShipSourceSnapshot<'source> {
        &self.sources
    }

    /// Borrows the combined original and variant source inventory for this result.
    #[must_use]
    pub const fn provenance(&self) -> CatalogProvenanceView<'_> {
        CatalogProvenanceView::new(self.sources.source_inventory(), &self.variant_sets)
    }
}

/// Supplies canonical Ships after comparing variants in the entire admissible Build.
///
/// # Errors
/// Returns input, original-evidence, Archive-change or cancellation errors.
pub fn with_ships<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ShipSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?.with_ships(selection, cancel, inspect)
}

/// Supplies canonical Ships from a decoded Entry bound to this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_ships_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ShipSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_ships(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies canonical Ships while reusing this session.
    ///
    /// # Errors
    /// Returns query, original-evidence, Archive-change or cancellation errors.
    pub fn with_ships<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ShipSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?.with_ships(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies canonical Ships while reusing this parsed DataForge.
    ///
    /// # Errors
    /// Returns query, original-evidence, Archive-change or cancellation errors.
    pub fn with_ships<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ShipSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            Query {
                archive,
                entry,
                forge,
                selection,
                languages: self.languages(),
                cancel,
            }
            .run_ships(inspect)
        })
    }
}
