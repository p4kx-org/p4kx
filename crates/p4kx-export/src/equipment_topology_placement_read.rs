// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeError};

use super::super::super::loadout_query::read::Query as LoadoutQuery;
use super::{ResourceTopologyPlacementSnapshot, ResourceTopologyRootState};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::catalog_v3::resource_topology_projection;
use crate::contract_loadout::EntityKind;
use crate::entity_selection::{self, Selected};
use crate::guid_text::CatalogGuid;
use crate::loadout::Wanted;
use crate::loadout::alignment::direct::DirectAlignment;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

#[path = "equipment_topology_placement_sources.rs"]
mod sources;

pub(super) fn run(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<ResourceTopologyPlacementSnapshot, CatalogError> {
    let origins = entry.records(archive)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let locale = TextSources::read_languages(archive, &[])?;
    let texts = Texts::parse(&locale)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Loadouts);
    let mut alignment = DirectAlignment::new(&projection, cancel);
    let mut coverage = CatalogRecordCoverage::default();
    let (wanted, root_state) = select(forge, ship_record_index, &mut coverage)?;
    let query = LoadoutQuery {
        archive,
        entry,
        forge,
        selection: CatalogRecordSelection::RecordIndex(ship_record_index),
        cancel,
    };
    let (mut rows, mut loadout_dependencies) = query.resolve_rows(
        &input,
        wanted.into_iter().collect(),
        &mut coverage,
        &mut alignment,
    )?;
    let loadout = rows.pop();
    let (originals, root_state) = match &loadout {
        Some(root) => (
            sources::read(root, &mut alignment, cancel)?,
            ResourceTopologyRootState::Resolved,
        ),
        None => (sources::Originals::default(), root_state),
    };
    loadout_dependencies.alignment = alignment.dependencies().collect();
    let placements = resource_topology_projection::project_scoped(
        forge,
        loadout.as_ref(),
        &originals.declarations,
        &originals.resources,
        &alignment.into_topology(),
        &origins.uses,
        cancel,
    )?;
    coverage.incomplete_fields.sort_unstable();
    coverage.incomplete_fields.dedup();
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    Ok(ResourceTopologyPlacementSnapshot {
        build: archive.build().cloned(),
        root_state,
        loadout,
        placements,
        coverage,
        loadout_dependencies,
        declaration_dependencies: originals.dependencies,
        source_inventory,
    })
}

fn select(
    forge: &DataForge<'_>,
    index: u32,
    coverage: &mut CatalogRecordCoverage,
) -> Result<(Option<Wanted>, ResourceTopologyRootState), CatalogError> {
    let Some(record) = forge.record(index) else {
        return Ok((None, ResourceTopologyRootState::MissingRecord));
    };
    match entity_selection::select(&record, None) {
        Ok(Some(Selected::Entity(Kind::Ships))) => Ok((
            Some(Wanted {
                record_index: index,
                guid: CatalogGuid(record.guid()),
                kind: EntityKind::Ship,
            }),
            ResourceTopologyRootState::ResolutionUnavailable,
        )),
        Ok(Some(Selected::Leftover(Kind::Ships))) => {
            coverage.skipped_leftovers += 1;
            Ok((None, ResourceTopologyRootState::NotAdmitted))
        }
        Ok(_) => Ok((None, ResourceTopologyRootState::NotAdmitted)),
        Err(DataForgeError::ObservationStopped) => Err(CatalogError::Cancelled),
        Err(_) => {
            coverage.unreadable_headers.push(index);
            Ok((None, ResourceTopologyRootState::UnreadableHeader))
        }
    }
}
