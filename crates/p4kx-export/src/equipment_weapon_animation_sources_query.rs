// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to the original Player Mannequin graph.

use p4kx_assembly::Entities;
use p4kx_extraction::{Archive, Build, CancelToken, DataForgeEntry, SourceInventory};

use super::EquipmentDependencyCoverage;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projections::weapon_reload;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;

pub use crate::catalog_projections::weapon_reload::{
    MannequinActorBinding, MannequinAssetLink, MannequinAssetState, MannequinBranch,
    MannequinBranchVisit, MannequinController, MannequinDatabase, MannequinDatabaseScopes,
    MannequinDeclaration, MannequinFragmentOption, MannequinFragmentSelector,
    MannequinFragmentStatus, MannequinNode, MannequinScope, MannequinScopeImport,
    MannequinSelectorNode, MannequinSubDatabase, MannequinTagDefinition, PlayerMotion,
    PlayerMotionComponent, WeaponAnimationGraphCoverage, WeaponAnimationSources,
};
pub use crate::catalog_read::loose_archive_path::{LooseArchivePath, LooseArchivePathState};
pub use crate::catalog_read::original::{
    PhysicalEntryIdentity, PhysicalEntryRef, SourceXmlAttribute,
};

/// The original graph reached from the physical Player Records of one Archive.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct WeaponAnimationSourcesSnapshot<'source> {
    /// Exact Build identified by the Archive when available.
    pub build: Option<&'source Build>,
    /// Availability of a readable database in the reached graph.
    pub availability: DatasetAvailability,
    /// Canonical authored graph within the Player scope.
    pub sources: WeaponAnimationSources<'source>,
    /// Physical source and use records referenced by these graph values.
    pub source_inventory: SourceInventory,
    /// Observations about the Player Records and graph scopes.
    pub graph_coverage: WeaponAnimationGraphCoverage,
    /// Failed required operations on reached Archive Entries.
    pub dependency_coverage: EquipmentDependencyCoverage,
}

/// Supplies the source graph reached from Player animation component locators.
///
/// The query opens original Player Records and their reached Mannequin documents.
/// It reads no Item bodies, localization or animation clip payloads.
///
/// # Errors
/// Returns input, source-accounting, Archive-change or cancellation errors.
pub fn with_player_weapon_animation_sources<T>(
    archive: &Archive,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponAnimationSourcesSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_player_weapon_animation_sources(cancel, inspect)
}

/// Supplies the same graph using the original decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_player_weapon_animation_sources_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponAnimationSourcesSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_player_weapon_animation_sources(cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies the Player graph through this Archive session.
    ///
    /// # Errors
    /// Returns input, source-accounting, Archive-change or cancellation errors.
    pub fn with_player_weapon_animation_sources<T>(
        &self,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponAnimationSourcesSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_player_weapon_animation_sources(cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies the Player graph through this parsed DataForge view.
    ///
    /// # Errors
    /// Returns source-accounting, Archive-change or cancellation errors.
    pub fn with_player_weapon_animation_sources<T>(
        &self,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponAnimationSourcesSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            let origins = CatalogOrigins::for_archive_records(archive, entry)?;
            let input = OriginalCatalogInput::new(forge, &origins);
            let players = Entities::records_of_class(forge, "Player");
            checkpoint(cancel)?;
            let projected = weapon_reload::project_sources(archive, &players, &input, cancel)?;
            let graph_coverage = projected.sources.graph_coverage(players.len());
            let mut dependency_coverage = EquipmentDependencyCoverage::default();
            dependency_coverage.finish(archive, &projected.ledger)?;
            let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
            checkpoint(cancel)?;
            Ok(inspect(WeaponAnimationSourcesSnapshot {
                build: archive.build(),
                availability: projected.availability,
                sources: projected.sources,
                source_inventory,
                graph_coverage,
                dependency_coverage,
            }))
        })
    }
}
