// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForgeEntry, SourceInventory, SourceValueState,
};

use super::{LoadoutDependencyCoverage, read::Query, scope};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::facility_loadout_placements::{
    collect_room_attachments, project_room_coverage,
};
use crate::contract_loadout::Loadout;
use crate::loadout::Wanted;
use crate::loadout::alignment::direct::DirectAlignment;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "equipment_facility_interior_sources.rs"]
mod sources;

pub use crate::catalog_v3::facility_loadout_placements::InteriorRoomCoverage;
pub use sources::InteriorCoverageDependency;
use sources::{ClassReads, RoomReads};

/// Original room coverage reached from one physical Ship.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct FacilityInteriorCoverageSnapshot {
    /// Build of the queried Archive.
    pub build: Option<Build>,
    /// Physical Record selector supplied to the query.
    pub ship_record_index: u32,
    /// Whether this physical root belongs to the canonical Ship selection.
    pub root_state: SourceValueState,
    /// Whether a Loadout could supply additional attachment paths.
    pub loadout_state: SourceValueState,
    /// Direct room declarations followed by attached rooms in original port order.
    pub rows: Vec<InteriorRoomCoverage>,
    /// Whether the selected scope and original room traversals were complete.
    pub complete: bool,
    /// Original Ship admission and Loadout coverage.
    pub loadout_coverage: CatalogRecordCoverage,
    /// Resolver and alignment coverage of the selected Loadout.
    pub loadout_dependencies: LoadoutDependencyCoverage,
    /// Physical Record dependencies of declaration and class selection.
    pub dependencies: Vec<InteriorCoverageDependency>,
    /// One source inventory for the selected scope.
    pub source_inventory: SourceInventory,
}

/// Reads interior room coverage for one physical Ship.
///
/// # Errors
/// Returns input, source evidence, Archive-change or cancellation errors.
pub fn read_facility_interior_coverage(
    archive: &Archive,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<FacilityInteriorCoverageSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_facility_interior_coverage(ship_record_index, cancel)
}

/// Reads interior room coverage using decoded DataForge bytes from the same Archive.
///
/// # Errors
/// Returns query errors or an input-origin mismatch.
pub fn read_facility_interior_coverage_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<FacilityInteriorCoverageSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_facility_interior_coverage(ship_record_index, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads interior room coverage through the current session.
    ///
    /// # Errors
    /// Returns input, source evidence, Archive-change or cancellation errors.
    pub fn read_facility_interior_coverage(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<FacilityInteriorCoverageSnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_facility_interior_coverage(ship_record_index, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads interior room coverage through the parsed DataForge.
    ///
    /// # Errors
    /// Returns input, source evidence, Archive-change or cancellation errors.
    pub fn read_facility_interior_coverage(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<FacilityInteriorCoverageSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            selected(
                &Query {
                    archive,
                    entry,
                    forge,
                    selection: CatalogRecordSelection::RecordIndex(ship_record_index),
                    cancel,
                },
                ship_record_index,
            )
        })
    }
}

fn selected(
    query: &Query<'_>,
    ship_record_index: u32,
) -> Result<FacilityInteriorCoverageSnapshot, CatalogError> {
    let origins = CatalogOrigins::for_archive_records(query.archive, query.entry)?;
    let input = OriginalCatalogInput::new(query.forge, &origins);
    let locales = TextSources::read_languages(query.archive, &[])?;
    let texts = Texts::parse(&locales)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Loadouts);
    let mut alignment = DirectAlignment::new(&projection, query.cancel);
    let loadout = resolve_loadout(query, &input, ship_record_index, &mut alignment)?;
    let (rows, dependencies) = read_room_scope(
        query,
        &input,
        loadout.root,
        loadout.row.as_ref(),
        &mut alignment,
    )?;
    let complete = loadout.complete
        && crate::export_archive_health::inventory(query.archive).is_none()
        && dependencies
            .iter()
            .copied()
            .all(InteriorCoverageDependency::is_complete)
        && rows.iter().all(InteriorRoomCoverage::is_complete);
    let source_inventory = origins.uses.finish_with(|| checkpoint(query.cancel))?;
    checkpoint(query.cancel)?;
    Ok(FacilityInteriorCoverageSnapshot {
        build: query.archive.build().cloned(),
        ship_record_index,
        root_state: loadout.root_state,
        loadout_state: loadout.state,
        rows,
        complete,
        loadout_coverage: loadout.coverage,
        loadout_dependencies: loadout.dependencies,
        dependencies,
        source_inventory,
    })
}

struct ResolvedLoadout {
    root: Option<Wanted>,
    root_state: SourceValueState,
    row: Option<Loadout>,
    state: SourceValueState,
    complete: bool,
    coverage: CatalogRecordCoverage,
    dependencies: LoadoutDependencyCoverage,
}

fn resolve_loadout(
    query: &Query<'_>,
    input: &OriginalCatalogInput<'_, '_>,
    ship: u32,
    alignment: &mut DirectAlignment<'_>,
) -> Result<ResolvedLoadout, CatalogError> {
    let scope::Scope {
        wanted,
        mut coverage,
    } = scope::read_ship(query.forge, ship, query.cancel)?;
    let root = wanted.first().copied();
    let root_state = if root.is_some() {
        SourceValueState::Present
    } else if coverage.unreadable_headers.is_empty() {
        SourceValueState::Absent
    } else {
        SourceValueState::Unreadable
    };
    let (mut rows, dependencies) = query.resolve_rows(input, wanted, &mut coverage, alignment)?;
    if rows.len() > 1 {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let row = rows.pop();
    coverage.incomplete_fields.sort_unstable();
    coverage.incomplete_fields.dedup();
    let complete = coverage.unreadable_headers.is_empty()
        && coverage.unreadable_bodies.is_empty()
        && coverage.incomplete_fields.is_empty()
        && dependencies.is_complete();
    let state = if row.is_some() {
        SourceValueState::Present
    } else if complete {
        SourceValueState::Absent
    } else {
        SourceValueState::Unreadable
    };
    Ok(ResolvedLoadout {
        root,
        root_state,
        row,
        state,
        complete,
        coverage,
        dependencies,
    })
}

fn read_room_scope(
    query: &Query<'_>,
    input: &OriginalCatalogInput<'_, '_>,
    root: Option<Wanted>,
    loadout: Option<&Loadout>,
    alignment: &mut DirectAlignment<'_>,
) -> Result<(Vec<InteriorRoomCoverage>, Vec<InteriorCoverageDependency>), CatalogError> {
    let mut room_reads = RoomReads::default();
    let mut class_reads = ClassReads::default();
    let rows = if let Some(root) = root {
        room_reads.read(root.record_index, alignment)?;
        let attachments = if let Some(loadout) = loadout {
            collect_room_attachments(loadout, input.origins(), query.cancel, &mut |index| {
                room_reads.read(index, alignment)
            })?
        } else {
            Vec::new()
        };
        project_room_coverage(
            query.archive,
            input,
            root,
            &room_reads.declarations,
            &attachments,
            query.cancel,
            &mut |index| class_reads.read(index, alignment),
        )?
    } else {
        Vec::new()
    };
    Ok((rows, sources::dependencies(room_reads, class_reads)))
}
