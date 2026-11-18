// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::snapshot::EquipmentSnapshot;
use super::{read, rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

pub use crate::catalog_v3::ammo_definition::{AmmoDefinition, AmmoSelection};

/// Selected original ammunition definitions and their source and text context.
pub type AmmoDefinitionSnapshot<'source> = EquipmentSnapshot<'source, AmmoDefinition<'source>>;

/// Supplies independently admitted original Ammo definitions in canonical order.
/// Publish consumer changes after outer success and the required coverage checks.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_ammo_definitions<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(AmmoDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::AmmoDefinitions, T>(
        archive, None, selection, languages, cancel, inspect,
    )
}

/// Supplies selected original Ammo definitions from an already decoded original Archive Entry.
///
/// # Errors
/// Returns query errors or an origin mismatch for a foreign Entry.
pub fn with_ammo_definitions_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(AmmoDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::AmmoDefinitions, T>(
        archive,
        Some(entry),
        selection,
        languages,
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies selected original Ammo definitions through this Archive session.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_ammo_definitions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(AmmoDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_ammo_definitions(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected original Ammo definitions through this parsed DataForge view.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_ammo_definitions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(AmmoDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<rows::AmmoDefinitions, T>(
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
