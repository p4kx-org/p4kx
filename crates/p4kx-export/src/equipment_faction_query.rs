// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to physical faction definitions and their authored text domains.

use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry, RecordTree};

use super::read;
use super::rows::{Family, Projector, ReadOptions};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::faction_definition;
use crate::record_observation::RecordObservation;

pub use crate::catalog_v3::faction_definition::{
    FactionDefinition, FactionText, FriendlyFireOverride,
};

/// Original faction rules, linked header evidence and requested presentation text.
pub type FactionSnapshot<'source> = super::EquipmentSnapshot<'source, FactionDefinition<'source>>;

/// Reads selected physical Faction Records and their original text references.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_factions<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(FactionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<Factions, T>(archive, None, selection, languages, cancel, inspect)
}

/// Reads selected Factions from this Archive's already decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_factions_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(FactionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<Factions, T>(archive, Some(entry), selection, languages, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Reads original Factions using this session and its locale policy.
    ///
    /// # Errors
    /// Returns source-evidence, Archive-change or cancellation errors.
    pub fn with_factions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(FactionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_factions(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads original Factions using the session's parsed DataForge.
    ///
    /// # Errors
    /// Returns source-evidence, Archive-change or cancellation errors.
    pub fn with_factions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(FactionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<Factions, T>(
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

struct Factions;

impl Projector for Factions {
    type Row<'source> = FactionDefinition<'source>;
    const FAMILY: Family = Family::Factions;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let read = faction_definition::read_selected(tree, projection, admission, cancel)?;
        Ok(read.map(|row| vec![row]))
    }
}
