// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry, RecordTree};

use super::read;
use super::rows::{ConfigurationFamily, Family, Projector, ReadOptions};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_entity_configurations;
use crate::record_observation::RecordObservation;

pub use crate::catalog_v3::catalog_interactions::InteractionConfiguration;
pub use crate::catalog_v3::interactable_configuration::InteractableConfiguration;
pub use crate::catalog_v3::interaction_point::InteractionPoint;
pub use crate::catalog_v3::interaction_state::InteractionState;
pub use crate::catalog_v3::interaction_state_edges::InteractionTagList;
pub use crate::catalog_v3::interaction_state_type::InteractionStateType;
pub use crate::catalog_v3::shared_interaction::SharedInteraction;

/// Selected original interactions, reached protected texts and source evidence.
pub type InteractionSnapshot<'source> =
    super::EquipmentSnapshot<'source, InteractionConfiguration<'source>>;

/// Reads authored interactions by physical Entity Record, GUID or complete family.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_interactions<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(InteractionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<Interactions, T>(archive, None, selection, languages, cancel, inspect)
}

/// Reads selected interactions from this Archive's decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_interactions_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(InteractionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<Interactions, T>(
        archive,
        Some(entry),
        selection,
        languages,
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Reads original interaction components using this session and its locale policy.
    ///
    /// # Errors
    /// Returns source-evidence, Archive-change or cancellation errors.
    pub fn with_interactions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(InteractionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_interactions(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads original interaction components from the session's parsed DataForge.
    ///
    /// # Errors
    /// Returns source-evidence, Archive-change or cancellation errors.
    pub fn with_interactions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(InteractionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<Interactions, T>(
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

struct Interactions;

impl Projector for Interactions {
    type Row<'source> = InteractionConfiguration<'source>;
    const FAMILY: Family = Family::Configuration(ConfigurationFamily::Interactions);

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        options: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        catalog_entity_configurations::read_interactions(
            tree,
            projection,
            admission,
            &options.alignment,
            cancel,
        )
    }
}
