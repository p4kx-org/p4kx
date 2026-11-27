// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Scoped compatibility of physical Paint Items and selected Ships.

use p4kx_assembly::PaintTagIndex;
use p4kx_extraction::{Archive, Build, CancelToken, FieldOrigin, Guid, SourceInventory};

use super::EquipmentDependencyCoverage;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::tag_catalog;
use crate::paint::ShipPaintMatches;
use crate::paint::{PaintCandidate, PaintRequirements};
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

pub use crate::contract_entities::Paint;

#[path = "paint_compatibility_fleet.rs"]
mod fleet;
#[path = "paint_compatibility_owners.rs"]
mod owners;
#[path = "paint_compatibility_record_inputs.rs"]
mod record_inputs;

use owners::Survey;

/// One original Paint owner in this DataForge.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum PaintSelection {
    /// Selects its exact physical Record position.
    RecordIndex(u32),
    /// Selects one Paint GUID when the canonical physical owner is unique.
    Guid(Guid),
}

/// Result of an exact original Paint compatibility scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PaintCompatibilityStatus {
    /// One canonical row has complete input coverage.
    Found,
    /// Complete original observations exclude an admitted Paint.
    NotFound,
    /// More than one admitted physical Paint has the requested GUID.
    Ambiguous,
    /// An observed row is available, with incomplete compatibility inputs.
    Partial,
    /// A row could not be selected with the available original inputs.
    Unavailable,
}

/// Scoped canonical compatibility and its original inputs.
#[derive(Debug)]
#[non_exhaustive]
pub struct PaintCompatibilitySnapshot {
    /// Archive Build, when identified.
    pub build: Option<Build>,
    /// Outcome of the requested Paint scope.
    pub status: PaintCompatibilityStatus,
    /// Canonical row, with any observed partial membership described by status.
    pub row: Option<Paint>,
    /// Observed physical Paint owners of the selected identity.
    pub paint_record_indices: Vec<u32>,
    /// Fitting Ship Record indices in this Build's DataForge, aligned with the row's Ship GUIDs.
    /// Equal GUIDs retain their physical Record order. A result without a row has no indices.
    pub ship_record_indices: Vec<u32>,
    /// Source scope used to select the Paint and its original attachment.
    pub paint_inputs: FieldOrigin,
    /// Original contributing compatibility inputs, including the selected Ship inventory.
    pub compatibility_inputs: Option<FieldOrigin>,
    /// Unreadable and incomplete Paint Record observations.
    pub paint_records: CatalogRecordCoverage,
    /// Unreadable and incomplete Ship Record observations when a fleet was read.
    pub ship_records: Option<CatalogRecordCoverage>,
    /// Actual vehicle dependency coverage when a fleet was read.
    pub vehicle_dependencies: Option<EquipmentDependencyCoverage>,
    /// Original source evidence used by this snapshot.
    pub source_inventory: SourceInventory,
}

/// Reads the canonical reverse fit of one selected Paint directly from the Archive.
///
/// # Errors
/// Returns errors for invalid source authority, changed Archive, or cancellation.
pub fn read_paint_compatibility(
    archive: &Archive,
    selection: PaintSelection,
    cancel: Option<&CancelToken>,
) -> Result<PaintCompatibilitySnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_paint_compatibility(selection, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads the exact Paint scope through the reusable Archive session.
    ///
    /// # Errors
    /// Returns source, Archive-change and cancellation errors.
    pub fn read_paint_compatibility(
        &self,
        selection: PaintSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<PaintCompatibilitySnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_paint_compatibility(selection, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    pub(crate) fn with_paint_matches<T, E: From<CatalogError>>(
        &self,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(&ShipPaintMatches, &PaintTagIndex<'_>, bool) -> Result<T, E>,
    ) -> Result<T, E> {
        self.project_fallible(cancel, |archive, entry, forge| {
            let origins = entry.records(archive)?;
            let input = OriginalCatalogInput::new(forge, &origins);
            let tag_catalog::TagContextRead {
                context: tags,
                failures,
            } = tag_catalog::read_context(forge, &input, cancel)?;
            let index = PaintTagIndex::new(tags.tree());
            let fleet = fleet::read(archive, &input, cancel, None)?;
            let coverage = complete(&fleet.records)
                && fleet.dependencies.is_complete()
                && failures.is_empty()
                && tags.paint_scope()?.source_set_complete()
                && fleet.origins.iter().all(FieldOrigin::source_set_complete);
            checkpoint(cancel)?;
            inspect(&fleet.matches, &index, coverage)
        })
    }
    /// Reads the exact Paint scope through the already parsed original DataForge.
    ///
    /// # Errors
    /// Returns source, Archive-change and cancellation errors.
    pub fn read_paint_compatibility(
        &self,
        selection: PaintSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<PaintCompatibilitySnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            let origins = entry.records(archive)?;
            let input = OriginalCatalogInput::new(forge, &origins);
            snapshot(archive, &input, selection, cancel)
        })
    }
}

struct Fit {
    ship_record_indices: Vec<u32>,
    row: Paint,
    origin: FieldOrigin,
    records: CatalogRecordCoverage,
    dependencies: EquipmentDependencyCoverage,
}

fn snapshot<'source>(
    archive: &'source Archive,
    input: &'source OriginalCatalogInput<'source, 'source>,
    selection: PaintSelection,
    cancel: Option<&CancelToken>,
) -> Result<PaintCompatibilitySnapshot, CatalogError> {
    let languages = TextSources::read_languages(archive, &[])?;
    let texts = Texts::parse(&languages)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, input, &registry).for_kind(Kind::Items);
    let Survey {
        candidates,
        origin,
        records,
    } = owners::read(&projection, selection, cancel)?;
    let paint_record_indices = candidates.iter().map(|owner| owner.index).collect();
    let mut status = match candidates.len() {
        0 if origin.source_set_complete() => PaintCompatibilityStatus::NotFound,
        2.. => PaintCompatibilityStatus::Ambiguous,
        _ => PaintCompatibilityStatus::Unavailable,
    };
    let (row, ship_record_indices, compatibility_inputs, ship_records, vehicle_dependencies) =
        match candidates.as_slice() {
            [owner] if owner.inputs.origin.source_set_complete() => {
                let fit = project(archive, input, owner, &origin, cancel)?;
                status = if fit.origin.source_set_complete() {
                    PaintCompatibilityStatus::Found
                } else {
                    PaintCompatibilityStatus::Partial
                };
                (
                    Some(fit.row),
                    fit.ship_record_indices,
                    Some(fit.origin),
                    Some(fit.records),
                    Some(fit.dependencies),
                )
            }
            _ => (None, Vec::new(), None, None, None),
        };
    checkpoint(cancel)?;
    Ok(PaintCompatibilitySnapshot {
        build: archive.build().cloned(),
        status,
        row,
        paint_record_indices,
        ship_record_indices,
        paint_inputs: origin,
        compatibility_inputs,
        paint_records: records,
        ship_records,
        vehicle_dependencies,
        source_inventory: input.origins().uses.finish_with(|| checkpoint(cancel))?,
    })
}

fn project<'source>(
    archive: &'source Archive,
    input: &'source OriginalCatalogInput<'source, 'source>,
    owner: &owners::Owner<'source>,
    paint: &FieldOrigin,
    cancel: Option<&CancelToken>,
) -> Result<Fit, CatalogError> {
    let tag_catalog::TagContextRead {
        context: tags,
        failures,
    } = tag_catalog::read_context(input.forge(), input, cancel)?;
    let taxonomy = tags.paint_scope()?;
    let requirements = PaintRequirements::new(&owner.inputs.tags, &owner.inputs.required_tags);
    let index = PaintTagIndex::new(tags.tree());
    let fleet::Fleet {
        matches,
        records,
        dependencies,
        mut origins,
    } = fleet::read(
        archive,
        input,
        cancel,
        Some(fleet::Selection {
            requirements: &requirements,
            index: &index,
        }),
    )?;
    origins.push(owners::record_inventory(input)?);
    origins.push(taxonomy);
    let sources = &input.origins().uses;
    let directory = sources.observe_root_directory_paths(archive)?;
    origins.push(sources.origin(directory.as_slice(), dependencies.directory_complete()));
    let complete = complete(&records) && dependencies.is_complete() && failures.is_empty();
    let origin =
        sources.origin_with_origins(origins.iter().chain(std::iter::once(paint)), complete)?;
    let candidate = PaintCandidate::from_authored(
        owner.index,
        owner.inputs.guid,
        &owner.inputs.tags,
        &owner.inputs.required_tags,
    );
    let (row, ordered_ships) = candidate.project_accepted(matches.owners().collect(), cancel)?;
    Ok(Fit {
        ship_record_indices: ordered_ships
            .into_iter()
            .map(|ship| ship.record_index)
            .collect(),
        row,
        origin,
        records,
        dependencies,
    })
}

const fn complete(coverage: &CatalogRecordCoverage) -> bool {
    coverage.unreadable_headers.is_empty()
        && coverage.unreadable_bodies.is_empty()
        && coverage.incomplete_fields.is_empty()
}
