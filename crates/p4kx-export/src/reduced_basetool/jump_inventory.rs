// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use crate::catalog_queries::starmap::{
    ContainerInstanceKey, JumpConnectionInventory, JumpEndpoint,
};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;
use crate::guid_text::CatalogGuid;

pub(super) const ENDPOINTS: &str = "placements.jumpEndpoints";
pub(super) const CONNECTIONS: &str = "projections.jumpConnections";

pub(super) struct JumpInventory {
    pub(super) endpoints: StagedCatalogTable,
    pub(super) connections: StagedCatalogTable,
}

pub(super) fn stage(
    endpoint_availability: DatasetAvailability,
    endpoints: &[JumpEndpoint],
    connection_inventory: &JumpConnectionInventory,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<JumpInventory, ReducedError> {
    require_available_scope(ENDPOINTS, endpoint_availability)?;
    let connection_availability = connection_inventory.availability();
    require_available_scope(CONNECTIONS, connection_availability)?;
    let mut rows = StagedCatalogRows::new(scratch, cancel);
    for endpoint in endpoints {
        rows.append(&Endpoint::read(endpoint)?)?;
    }
    let staged = rows.finish(endpoint_availability)?;
    completeness::rows(ENDPOINTS, endpoints.len(), staged.len())?;
    let connections = StagedCatalogRows::new(scratch, cancel).finish(connection_availability)?;
    completeness::rows(
        CONNECTIONS,
        connection_inventory.rows().len(),
        connections.len(),
    )?;
    Ok(JumpInventory {
        endpoints: staged,
        connections,
    })
}

fn require_available_scope(
    path: &'static str,
    availability: DatasetAvailability,
) -> Result<(), ReducedError> {
    if matches!(availability, DatasetAvailability::Unavailable) {
        return Err(ReducedError::IncompleteScope(path));
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Endpoint<'read> {
    instance_id: Cell<&'read ContainerInstanceKey>,
    system: Cell<&'read str>,
    starmap_record_guid: Cell<&'read CatalogGuid>,
    starmap_record_index: Cell<&'read u32>,
    starmap_type: Cell<&'read str>,
    world_position: Cell<&'read [f64; 3]>,
    target_path: Cell<&'read str>,
    target_candidates: Cell<Vec<Cell<Entry>>>,
    declared_partner: Cell<&'read CatalogGuid>,
}

impl<'read> Endpoint<'read> {
    fn read(value: &'read JumpEndpoint) -> Result<Self, ReducedError> {
        Ok(Self {
            instance_id: completeness::cell(ENDPOINTS, value.instance_id())?,
            system: completeness::cell(ENDPOINTS, value.system())?.map(String::as_str),
            starmap_record_guid: completeness::cell(ENDPOINTS, value.starmap_record_guid())?,
            starmap_record_index: completeness::cell(ENDPOINTS, value.starmap_record_index())?,
            starmap_type: completeness::cell(ENDPOINTS, value.starmap_type())?.map(|name| *name),
            world_position: completeness::cell(ENDPOINTS, value.world_position())?,
            target_path: completeness::cell(ENDPOINTS, value.target_path())?.map(String::as_str),
            target_candidates: completeness::list(ENDPOINTS, value.target_candidates(), |entry| {
                Ok(Entry {
                    entry_index: entry.entry_index(),
                })
            })?,
            declared_partner: completeness::cell(ENDPOINTS, value.declared_partner())?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    entry_index: u32,
}
