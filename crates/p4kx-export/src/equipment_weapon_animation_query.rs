// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::Entities;
use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use super::EquipmentDependencyCoverage;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_projections::weapon_reload;
use crate::catalog_queries::unlocalized_record_query::plan::Plan;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::catalog_item_components::definition_read;
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

pub use crate::catalog_projections::weapon_reload::{
    Playback as WeaponAnimationPlayback, TagEvidence as WeaponAnimationTagEvidence,
    TagMatch as WeaponAnimationTagMatch, WeaponAnimationGraphCoverage,
    WeaponPlayerAnimationCandidate,
};

/// Scoped original animation candidates with owner and reached source coverage.
#[derive(Debug)]
#[non_exhaustive]
pub struct WeaponAnimationSnapshot<'archive> {
    /// Exact Build identified when the Archive was opened, if available.
    pub build: Option<&'archive Build>,
    /// Availability of the reached Player database for the selected Item.
    pub availability: DatasetAvailability,
    /// The canonical candidates for this Item in original order.
    pub rows: Vec<WeaponPlayerAnimationCandidate>,
    /// The physical sources and uses for the result.
    pub source_inventory: SourceInventory,
    /// Coverage of the selected Item Record and its authored fields.
    pub coverage: CatalogRecordCoverage,
    /// Coverage of the reached Player graph and selectors.
    pub graph_coverage: WeaponAnimationGraphCoverage,
    /// Observed failures of required Archive Entry operations.
    pub dependency_coverage: EquipmentDependencyCoverage,
}

/// Supplies authored Player animation candidates for one physical Item.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_weapon_animation_candidates<T>(
    archive: &Archive,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponAnimationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_weapon_animation_candidates(record_index, cancel, inspect)
}

/// Supplies the same scoped candidates using an original decoded Archive Entry.
///
/// # Errors
/// Returns query errors or an origin mismatch for a foreign Entry.
pub fn with_weapon_animation_candidates_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponAnimationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_weapon_animation_candidates(record_index, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies the selected Item's candidates through this Archive session.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_weapon_animation_candidates<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponAnimationSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_weapon_animation_candidates(record_index, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies the selected Item's candidates through the parsed DataForge view.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_weapon_animation_candidates<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponAnimationSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            with_owner(archive, entry, forge, record_index, cancel, inspect)
        })
    }
}

fn with_owner<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponAnimationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let mut plan = Plan::read(
        forge,
        Kind::Items,
        CatalogRecordSelection::RecordIndex(record_index),
        cancel,
        None,
    )?;
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let locale_sources = TextSources::read_languages(archive, &[])?;
    let texts = Texts::parse(&locale_sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Items);
    let mut attachments = Vec::new();
    for owner in plan.owners {
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let read = definition_read::read(&owner.tree, &projection, &admission, cancel)?;
        if !read.selection_complete
            || read
                .notes
                .iter()
                .any(|note| note.description().issue.blocks_projection())
        {
            plan.coverage.incomplete_fields.push(owner.record.index());
        }
        attachments.extend(read.value.weapon_attachments());
    }
    let players = if attachments.is_empty() {
        Vec::new()
    } else {
        Entities::records_of_class(forge, "Player")
    };
    checkpoint(cancel)?;
    let mut rows = Vec::new();
    let projected =
        weapon_reload::project(archive, &players, &input, &attachments, cancel, |row| {
            rows.push(row);
            Ok(())
        })?;
    let mut dependency_coverage = EquipmentDependencyCoverage::default();
    dependency_coverage.finish(archive, &projected.ledger)?;
    let graph_coverage = projected.sources.graph_coverage(players.len());
    drop(projected.sources);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(WeaponAnimationSnapshot {
        build: archive.build(),
        availability: projected.availability,
        rows,
        source_inventory,
        coverage: plan.coverage,
        graph_coverage,
        dependency_coverage,
    }))
}
