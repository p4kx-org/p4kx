// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{PathKeys, SourceInventory, SourceValueState};

use super::super::visit::OwnerLoadout;
use super::super::{LoadoutDependencyCoverage, read::Query, scope};
use super::sources::{self, ClassReads, RoomReads};
use super::{FacilityInteriorCoverageSnapshot, InteriorCoverageDependency, InteriorRoomCoverage};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::facility_instance::FacilityInstance;
use crate::catalog_v3::facility_loadout_placements::{
    RoomProjection, RoomScope, collect_room_attachments, project_rooms,
};
use crate::contract_loadout::Loadout;
use crate::loadout::Wanted;
use crate::loadout::alignment::direct::DirectAlignment;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, ExportAccountingError, Kind};

pub(super) fn selected(
    query: &Query<'_>,
    ship_record_index: u32,
    include_placements: bool,
) -> Result<(FacilityInteriorCoverageSnapshot, Vec<FacilityInstance>), CatalogError> {
    let origins = query.entry.records(query.archive)?;
    let input = OriginalCatalogInput::new(query.forge, &origins);
    let locales = TextSources::read_languages(query.archive, &[])?;
    let texts = Texts::parse(&locales)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Loadouts);
    let mut alignment = DirectAlignment::new(&projection, query.cancel);
    let loadout = resolve_loadout(query, &input, ship_record_index, &mut alignment)?;
    let rooms = read_room_scope(
        query,
        &input,
        loadout.root,
        loadout.row.as_ref(),
        &mut alignment,
        include_placements,
        &mut None,
    )?;
    let source_inventory = origins.uses.finish_with(|| checkpoint(query.cancel))?;
    checkpoint(query.cancel)?;
    let archive_complete = crate::export_archive_health::inventory(query.archive).is_none();
    Ok(finish_scope(
        query,
        archive_complete,
        ship_record_index,
        loadout,
        rooms,
        source_inventory,
    ))
}

pub(super) fn finish_scope(
    query: &Query<'_>,
    archive_complete: bool,
    ship_record_index: u32,
    loadout: ResolvedLoadout,
    rooms: ReadRooms,
    source_inventory: SourceInventory,
) -> (FacilityInteriorCoverageSnapshot, Vec<FacilityInstance>) {
    let complete = loadout.complete
        && archive_complete
        && rooms
            .dependencies
            .iter()
            .copied()
            .all(InteriorCoverageDependency::is_complete)
        && rooms
            .projected
            .coverage
            .iter()
            .all(InteriorRoomCoverage::is_complete);
    let coverage = FacilityInteriorCoverageSnapshot {
        build: query.archive.build().cloned(),
        ship_record_index,
        root_state: loadout.root_state,
        loadout_state: loadout.state,
        rows: rooms.projected.coverage,
        complete,
        loadout_coverage: loadout.coverage,
        loadout_dependencies: loadout.dependencies,
        dependencies: rooms.dependencies,
        source_inventory,
    };
    (coverage, rooms.projected.placements)
}

pub(super) struct ResolvedLoadout {
    root: Option<Wanted>,
    root_state: SourceValueState,
    row: Option<Loadout>,
    state: SourceValueState,
    complete: bool,
    coverage: CatalogRecordCoverage,
    dependencies: LoadoutDependencyCoverage,
}

impl ResolvedLoadout {
    pub(super) fn new(
        root: Option<Wanted>,
        root_state: SourceValueState,
        owner: OwnerLoadout,
        mut coverage: CatalogRecordCoverage,
    ) -> Self {
        let OwnerLoadout { row, dependencies } = owner;
        scope::finish_coverage(&mut coverage);
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
        Self {
            root,
            root_state,
            row,
            state,
            complete,
            coverage,
            dependencies,
        }
    }
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
    let owner = OwnerLoadout {
        row: rows.pop(),
        dependencies,
    };
    Ok(ResolvedLoadout::new(root, root_state, owner, coverage))
}

#[derive(Default)]
pub(super) struct ReadRooms {
    projected: RoomProjection,
    dependencies: Vec<InteriorCoverageDependency>,
}

pub(super) fn read_room_scope(
    query: &Query<'_>,
    input: &OriginalCatalogInput<'_, '_>,
    root: Option<Wanted>,
    loadout: Option<&Loadout>,
    alignment: &mut DirectAlignment<'_>,
    include_placements: bool,
    path_keys: &mut Option<PathKeys>,
) -> Result<ReadRooms, CatalogError> {
    let Some(root) = root else {
        return Ok(ReadRooms::default());
    };
    let mut room_reads = RoomReads::default();
    room_reads.read(root.record_index, alignment)?;
    let attachments = if let Some(loadout) = loadout {
        collect_room_attachments(loadout, input.origins(), query.cancel, &mut |index| {
            room_reads.read(index, alignment)
        })?
    } else {
        Vec::new()
    };
    let scope = RoomScope {
        root,
        declarations: &room_reads.declarations,
        attachments: &attachments,
    };
    let mut class_reads = ClassReads::new(alignment, include_placements);
    let projected = project_rooms(
        query.archive,
        input,
        scope,
        query.cancel,
        &mut class_reads,
        include_placements,
        path_keys,
    )?;
    Ok(ReadRooms {
        projected,
        dependencies: sources::dependencies(room_reads, class_reads),
    })
}
