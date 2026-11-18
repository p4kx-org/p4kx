// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::ShipSource;
use super::snapshot::EquipmentSnapshot;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

#[path = "equipment_ship_source_complete.rs"]
mod complete;
#[path = "equipment_ship_coverage.rs"]
mod coverage;
#[path = "equipment_ship_source_read.rs"]
mod read;

pub use coverage::{ShipSourceDependencyCoverage, ShipSourceEntryFailure, ShipSourceFailureStage};

/// Selected source-backed Ship fields and their resolver dependency coverage.
pub type ShipSourceSnapshot<'source> =
    EquipmentSnapshot<'source, ShipSource<'source>, ShipSourceDependencyCoverage>;

impl EquipmentSnapshot<'_, ShipSource<'_>, ShipSourceDependencyCoverage> {
    /// Borrows actual dependency read, projection and source-coordinate coverage.
    #[must_use]
    pub const fn dependency_coverage(&self) -> &ShipSourceDependencyCoverage {
        &self.dependency_coverage
    }
}

/// Supplies canonical original Ship fields plus actual resolved mass and hull cells.
/// Publish consumer-owned changes only after outer success and coverage checks.
///
/// # Errors
/// Returns input, original-evidence, Archive-change or cancellation errors.
pub fn with_ship_sources<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ShipSourceSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_ship_sources(selection, cancel, inspect)
}

/// Supplies the same Ship source cells from a decoded Entry bound to this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_ship_sources_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ShipSourceSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_ship_sources(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected canonical Ship source cells through this reusable session.
    ///
    /// # Errors
    /// Returns query, original-evidence, Archive-change or cancellation errors.
    pub fn with_ship_sources<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ShipSourceSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_ship_sources(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected canonical Ship cells while reusing this parsed DataForge.
    ///
    /// # Errors
    /// Returns query, original-evidence, Archive-change or cancellation errors.
    pub fn with_ship_sources<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ShipSourceSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::Query {
                archive,
                entry,
                forge,
                selection,
                languages: self.languages(),
                cancel,
            }
            .run(inspect)
        })
    }
}
