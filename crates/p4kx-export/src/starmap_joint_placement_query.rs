// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, Guid};

use super::super::object_placements::StarmapObjectPlacementCoverage;
use super::{JumpInventorySnapshot, finish_jumps, read_placements};
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::{CatalogRecordCoverage, ParsedCatalogQueries, starmap_name_owners};
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::catalog_v3::starmap_placement::StarmapPlacement;
use crate::guid_text::CatalogGuid;
use crate::starmap::ObjectPlacement;
use crate::starmap_containers::Placements;
use crate::{CatalogError, ExportAccountingError, Kind, StarmapTraversalBudget};

pub(crate) struct StarmapPlacementInventory {
    pub(crate) objects: CatalogTable<StarmapPlacement>,
    pub(crate) jumps: JumpInventorySnapshot,
}

impl ParsedCatalogQueries<'_> {
    pub(crate) fn read_starmap_placement_inventory(
        &self,
        owners: &[(u32, Guid)],
        budget: StarmapTraversalBudget,
        cancel: Option<&CancelToken>,
    ) -> Result<StarmapPlacementInventory, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            let origins = entry.records(archive)?;
            compose(archive, forge, &origins, owners, budget, cancel)
        })
    }
}

fn compose(
    archive: &Archive,
    forge: &DataForge<'_>,
    origins: &CatalogOrigins,
    owners: &[(u32, Guid)],
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
) -> Result<StarmapPlacementInventory, CatalogError> {
    let mut guids = owners.iter().map(|(_, guid)| *guid).collect::<Vec<_>>();
    guids.sort_unstable();
    guids.dedup();
    let (mut placements, candidates) =
        read_placements(archive, forge, origins, budget, cancel, Some(&guids))?;
    let coverage = StarmapObjectPlacementCoverage::measure(archive, &mut placements)?;
    let objects = if coverage.is_complete() {
        bound_objects(forge, owners, &placements, origins, cancel)?
    } else {
        CatalogTable::empty(DatasetAvailability::Unavailable)
    };
    let jumps = finish_jumps(
        archive,
        placements,
        &candidates,
        coverage.source_entries,
        origins,
        cancel,
    )?;
    Ok(StarmapPlacementInventory { objects, jumps })
}

fn bound_objects(
    forge: &DataForge<'_>,
    owners: &[(u32, Guid)],
    placements: &Placements,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<CatalogTable<StarmapPlacement>, CatalogError> {
    let mut coverage = CatalogRecordCoverage::default();
    let mut rows = Vec::new();
    for &(index, guid) in owners {
        checkpoint(cancel)?;
        let record = forge
            .record(index)
            .filter(|record| record.guid() == guid)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if !starmap_name_owners::selected(&record, None, &mut coverage) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let guid = CatalogGuid(guid);
        let placement = ObjectPlacement::select(guid, placements);
        if placement.position.is_some() {
            rows.try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
            rows.push(StarmapPlacement::from_placement(
                index, guid, placement, origins,
            )?);
        }
    }
    Ok(CatalogTable::new(
        forge,
        &[Kind::StarmapObjects.records().type_name],
        rows,
    ))
}
