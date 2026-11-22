// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use p4kx_extraction::{Archive, CancelToken, Guid};

use super::jump_class::JumpEndpointRecords;
use super::{Limits, PlacementScope, PlacementSpools, Placements};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::DatasetAvailability;
use crate::entry_source;
use crate::export_ledger::ExportLedger;
use crate::harvest_locations::HarvestSurvey;
use crate::starmap_index::Index;
use crate::starmap_parse::{DescriptorScope, Read};
use crate::starmap_walk::SystemRead;
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "starmap_container_systems.rs"]
mod systems;

#[derive(Clone, Copy)]
enum PlacementStorage<'directory> {
    Spool(&'directory Path),
    #[cfg(test)]
    Typed,
    Harvest(&'directory HarvestSurvey<'directory>),
    OpenedContainers,
    Object(Guid),
}

impl PlacementStorage<'_> {
    const fn descriptor_scope(self, jump_endpoints: bool) -> DescriptorScope {
        match self {
            Self::Harvest(_) => DescriptorScope::Harvest,
            Self::OpenedContainers | Self::Object(_) => DescriptorScope::OpenedContainers,
            Self::Spool(_) => DescriptorScope::Catalog { jump_endpoints },
            #[cfg(test)]
            Self::Typed => DescriptorScope::Catalog { jump_endpoints },
        }
    }
}

impl Placements {
    #[cfg(test)]
    pub(crate) fn read(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
    ) -> Result<Self, CatalogError> {
        Self::read_within(archive, cancel, Limits::default(), capture, scope)
    }

    pub(crate) fn read_for_catalog(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
        endpoints: &JumpEndpointRecords,
        directory: &Path,
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            scope,
            Some(endpoints),
            PlacementStorage::Spool(directory),
        )
    }

    #[cfg(test)]
    pub(crate) fn read_within(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            scope,
            None,
            PlacementStorage::Typed,
        )
    }

    pub(crate) fn read_for_harvest(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        harvest: &HarvestSurvey<'_>,
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            PlacementScope::RecordSelection,
            None,
            PlacementStorage::Harvest(harvest),
        )
    }

    pub(crate) fn read_loot_seeds(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            scope,
            None,
            PlacementStorage::OpenedContainers,
        )
    }

    pub(crate) fn read_for_object(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        guid: Guid,
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            PlacementScope::ArchiveInventory,
            None,
            PlacementStorage::Object(guid),
        )
    }

    fn empty_inventory(archive: &Archive, endpoint_inventory_complete: bool) -> Self {
        Self {
            instance_availability: Some(DatasetAvailability::Absent),
            spools: PlacementSpools {
                location_physics_availability: Some(crate::location_physics::stage::availability(
                    grid_directory_complete(archive),
                    false,
                )),
                ..PlacementSpools::default()
            },
            endpoint_inventory_complete,
            ..Self::default()
        }
    }

    fn read_scoped(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
        endpoints: Option<&JumpEndpointRecords>,
        storage: PlacementStorage<'_>,
    ) -> Result<Self, CatalogError> {
        check_cancelled(cancel)?;
        let started = Instant::now();
        let index = Index::new(archive);
        let systems = index.systems();
        if systems.is_empty() && matches!(scope, PlacementScope::ArchiveInventory) {
            return Ok(Self::empty_inventory(
                archive,
                endpoints.is_some_and(|records| records.complete),
            ));
        }
        let system_count = systems.len();
        let mut root_ledger = ExportLedger::default();
        let systems = unique_system_roots(archive, &index, capture, systems, &mut root_ledger)?;
        let roots: Vec<usize> = systems.iter().map(|(_, entry)| *entry).collect();
        let Read {
            bodies, mut ledger, ..
        } = crate::starmap_parse::read_all_scoped(
            archive,
            &index,
            &roots,
            cancel,
            (
                limits.source_children,
                storage.descriptor_scope(endpoints.is_some()),
            ),
            capture,
        )?;
        check_cancelled(cancel)?;
        let (read, mut spools) = systems::walk_systems(
            &index,
            &bodies,
            &systems,
            cancel,
            limits,
            (archive, capture, endpoints),
            storage,
        )?;
        drop(bodies);
        check_cancelled(cancel)?;
        let routes_complete = finish_route_coverage(&read, &mut ledger);
        ledger.merge(root_ledger);
        spools.finish_grid_scope(
            storage,
            archive,
            routes_complete && roots.len() == system_count,
            &ledger,
        );
        let mut placements = Self {
            ledger,
            spools,
            ..Self::default()
        };
        placements.finish(&index, system_count, read, scope, (archive, capture))?;
        placements.endpoint_inventory_complete =
            placements.has_complete_endpoints(routes_complete, endpoints);
        placements.report.elapsed = started.elapsed();
        Ok(placements)
    }

    fn has_complete_endpoints(
        &self,
        routes_complete: bool,
        endpoints: Option<&JumpEndpointRecords>,
    ) -> bool {
        routes_complete
            && !self
                .ledger
                .incomplete_inventories
                .contains(&Kind::StarmapObjects)
            && endpoints.is_some_and(|records| records.complete)
            && self.counts.unreadable == 0
            && self.counts.missing == 0
    }
}

impl PlacementSpools {
    fn finish_grid_scope(
        &mut self,
        storage: PlacementStorage<'_>,
        archive: &Archive,
        routes_complete: bool,
        ledger: &ExportLedger,
    ) {
        if !matches!(storage, PlacementStorage::Spool(_)) {
            return;
        }
        let readable = routes_complete
            && grid_directory_complete(archive)
            && !ledger
                .incomplete_inventories
                .contains(&Kind::StarmapObjects);
        if matches!(
            self.location_physics_availability,
            Some(DatasetAvailability::Available)
        ) {
            return;
        }
        if !readable {
            self.location_physics_availability = Some(DatasetAvailability::Unavailable);
        } else if self.location_physics_availability.is_none() {
            self.location_physics_availability = Some(DatasetAvailability::Absent);
        }
    }
}

fn grid_directory_complete(archive: &Archive) -> bool {
    crate::export_archive_health::inventory(archive).is_none()
}

pub(crate) fn finish_route_coverage(read: &[SystemRead], ledger: &mut ExportLedger) -> bool {
    if read.iter().any(|system| system.placement_budget_exhausted) {
        ledger.incomplete_inventory(Kind::StarmapObjects);
    }
    read.iter().all(|system| !system.routes_incomplete)
}

fn check_cancelled(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}

pub(crate) fn unique_system_roots(
    archive: &Archive,
    index: &Index<'_>,
    capture: &EntryCapture<'_>,
    roots: Vec<(Arc<str>, usize)>,
    ledger: &mut ExportLedger,
) -> Result<Vec<(Arc<str>, usize)>, CatalogError> {
    let mut unique = Vec::with_capacity(roots.len());
    for (name, index_entry) in roots {
        let entry = index
            .entries
            .get(index_entry)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        match index.candidates(entry.path()) {
            [entry] => unique.push((name, *entry)),
            [] => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
            candidates => entry_source::record_ambiguous_outer_entries(
                archive,
                capture,
                ledger,
                Kind::StarmapObjects,
                None,
                candidates,
            )?,
        }
    }
    Ok(unique)
}
