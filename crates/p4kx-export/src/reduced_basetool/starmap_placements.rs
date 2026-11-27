// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::jump_inventory::{self, JumpInventory};
use super::record_targets::RecordTargets;
use crate::StarmapTraversalBudget;
use crate::catalog_queries::ParsedCatalogQueries;
use crate::catalog_queries::starmap::{JumpEndpoint, StarmapPlacement};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::guid_text::CatalogGuid;

pub(super) const PATH: &str = "placements.starmapPlacements";

pub(super) struct StarmapPlacements {
    pub(super) placements: StagedCatalogTable,
    pub(super) jumps: JumpInventory,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    objects: &RecordTargets,
    budget: StarmapTraversalBudget,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StarmapPlacements, ReducedError> {
    let mut owners = objects.iter().collect::<Vec<_>>();
    owners.sort_by_cached_key(|(index, guid)| (CatalogGuid(*guid).text(), *index));
    let inventory = parsed.read_starmap_placement_inventory(&owners, budget, cancel)?;
    let placements = stage(&inventory.objects, objects, scratch, cancel)?;
    verify_endpoint_objects(&inventory.jumps.endpoints, objects)?;
    let jumps = jump_inventory::stage(
        inventory.jumps.endpoint_availability,
        &inventory.jumps.endpoints,
        &inventory.jumps.connections,
        scratch,
        cancel,
    )?;
    Ok(StarmapPlacements { placements, jumps })
}

fn verify_endpoint_objects(
    endpoints: &[JumpEndpoint],
    objects: &RecordTargets,
) -> Result<(), ReducedError> {
    let path = jump_inventory::ENDPOINTS;
    for endpoint in endpoints {
        let guid = completeness::cell(path, endpoint.starmap_record_guid())?;
        let index = completeness::cell(path, endpoint.starmap_record_index())?;
        let (Some(guid), Some(&index)) = (guid.value, index.value) else {
            return Err(ReducedError::IncompleteScope(path));
        };
        objects.verify(path, index, guid.get())?;
    }
    Ok(())
}

fn stage(
    source: &CatalogTable<StarmapPlacement>,
    objects: &RecordTargets,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogTable, ReducedError> {
    if matches!(source.availability, DatasetAvailability::Unavailable) {
        return Err(ReducedError::IncompleteScope(PATH));
    }
    let mut output = StagedCatalogRows::new(scratch, cancel);
    for original in &source.rows {
        output.append(&Placement::read(original, objects)?)?;
    }
    let table = output.finish(source.availability)?;
    completeness::rows(PATH, source.rows.len(), table.len())?;
    Ok(table)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Placement<'read> {
    object_guid: Cell<&'read CatalogGuid>,
    object_record_index: Cell<&'read u32>,
    system: Cell<&'read str>,
    entity_name: Cell<&'read str>,
    placed_under_guid: Cell<&'read CatalogGuid>,
    position: Cell<&'read [f64; 3]>,
}

impl<'read> Placement<'read> {
    fn read(
        source: &'read StarmapPlacement,
        objects: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        let object_guid = completeness::cell(PATH, source.object_guid())?;
        let object_record_index = completeness::cell(PATH, source.object_record_index())?;
        let (Some(guid), Some(&index)) = (object_guid.value, object_record_index.value) else {
            return Err(ReducedError::IncompleteScope(PATH));
        };
        objects.verify(PATH, index, guid.get())?;
        let position = completeness::cell(PATH, source.position())?;
        let system = completeness::cell(PATH, source.system())?.map(String::as_str);
        if system.value.is_none()
            || !position
                .value
                .is_some_and(|point| point.iter().all(|axis| axis.is_finite()))
        {
            return Err(ReducedError::IncompleteScope(PATH));
        }
        Ok(Self {
            object_guid,
            object_record_index,
            system,
            entity_name: completeness::cell(PATH, source.entity_name())?.map(String::as_str),
            placed_under_guid: completeness::cell(PATH, source.placed_under_guid())?,
            position,
        })
    }
}
