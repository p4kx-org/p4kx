// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::snapshot::MiningDefinitionSnapshot;
use super::{read, rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

pub use crate::catalog_v3::mineable_entity_definition::{
    MineableBaseSignature, MineableEntity, MineableParameters, MineableRadarProperties,
    MineableSignature,
};

/// Selected physical mineable entities with their own original evidence.
pub type MineableEntitySnapshot<'source> =
    MiningDefinitionSnapshot<'source, MineableEntity<'source>>;

/// Supplies mineable entities without building the Catalog or reading placements.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_mineable_entities<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MineableEntitySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Entities, T>(
        archive,
        None,
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

/// Supplies mineable entities from an already decoded Entry of the same Archive.
///
/// # Errors
/// Returns input, source-origin, source-evidence, Archive-change or cancellation errors.
pub fn with_mineable_entities_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MineableEntitySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Entities, T>(
        archive,
        Some(entry),
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies this Archive session's selected physical mineable entities.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_mineable_entities<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MineableEntitySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_mineable_entities(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected mineable entities using this view's retained indexes.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_mineable_entities<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MineableEntitySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<rows::Entities, T>(
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
