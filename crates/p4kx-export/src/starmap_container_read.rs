// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use p4kx_extraction::{Archive, CancelToken};
#[cfg(test)]
use rayon::prelude::*;

use super::jump_class::JumpEndpointRecords;
use super::{Limits, PlacementScope, PlacementSpools, Placements};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_rows::spool::RowSpoolWriter;
use crate::catalog_table::DatasetAvailability;
use crate::entry_source;
use crate::export_ledger::ExportLedger;
use crate::harvest_locations::HarvestSurvey;
use crate::starmap_index::Index;
use crate::starmap_parse::{Body, DescriptorScope, Read};
use crate::starmap_walk::{PlacementRows, System, SystemRead};
use crate::{CatalogError, ExportAccountingError, Kind};

#[derive(Clone, Copy)]
enum PlacementStorage<'directory> {
    Spool(&'directory Path),
    #[cfg(test)]
    Typed,
    Harvest(&'directory HarvestSurvey<'directory>),
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
            return Ok(Self {
                instance_availability: Some(DatasetAvailability::Absent),
                endpoint_inventory_complete: endpoints.is_some_and(|records| records.complete),
                ..Self::default()
            });
        }
        let system_count = systems.len();
        let mut root_ledger = ExportLedger::default();
        let systems = unique_system_roots(archive, &index, capture, systems, &mut root_ledger)?;
        let roots: Vec<usize> = systems.iter().map(|(_, entry)| *entry).collect();
        let descriptor_scope = if matches!(storage, PlacementStorage::Harvest(_)) {
            DescriptorScope::HARVEST
        } else {
            DescriptorScope::catalog(endpoints.is_some())
        };
        let Read {
            bodies, mut ledger, ..
        } = crate::starmap_parse::read_all_scoped(
            archive,
            &index,
            &roots,
            cancel,
            (limits.source_children, descriptor_scope),
            capture,
        )?;
        check_cancelled(cancel)?;
        let (read, spools) = walk_systems(
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
        let mut placements = Self {
            ledger,
            spools,
            ..Self::default()
        };
        placements.finish(&index, system_count, read, scope, (archive, capture))?;
        placements.endpoint_inventory_complete = routes_complete
            && !placements
                .ledger
                .incomplete_inventories
                .contains(&Kind::StarmapObjects)
            && endpoints.is_some_and(|records| records.complete)
            && placements.counts.unreadable == 0
            && placements.counts.missing == 0;
        placements.report.elapsed = started.elapsed();
        Ok(placements)
    }
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

fn walk_systems(
    index: &Index<'_>,
    bodies: &HashMap<usize, Body>,
    systems: &[(Arc<str>, usize)],
    cancel: Option<&CancelToken>,
    limits: Limits,
    (archive, capture, endpoints): (&Archive, &EntryCapture<'_>, Option<&JumpEndpointRecords>),
    storage: PlacementStorage<'_>,
) -> Result<(Vec<SystemRead>, PlacementSpools), CatalogError> {
    if systems.is_empty() {
        return Ok((Vec::new(), PlacementSpools::default()));
    }
    match storage {
        PlacementStorage::Spool(directory) => walk_spooled_systems(
            index,
            bodies,
            systems,
            cancel,
            limits,
            (archive, capture, endpoints),
            directory,
        ),
        PlacementStorage::Harvest(harvest) => {
            let reads = systems
                .iter()
                .map(|(name, entry)| {
                    System::run_with_output(
                        index,
                        bodies,
                        name,
                        *entry,
                        cancel,
                        limits,
                        (archive, capture, None, PlacementRows::Harvest(harvest)),
                    )
                })
                .collect::<Result<_, _>>()?;
            Ok((reads, PlacementSpools::default()))
        }
        #[cfg(test)]
        PlacementStorage::Typed => {
            let reads = systems
                .par_iter()
                .map(|(name, entry)| {
                    System::run(
                        index,
                        bodies,
                        name,
                        *entry,
                        cancel,
                        limits,
                        (archive, capture, endpoints),
                    )
                })
                .collect::<Result<_, _>>()?;
            Ok((reads, PlacementSpools::default()))
        }
    }
}

fn walk_spooled_systems(
    index: &Index<'_>,
    bodies: &HashMap<usize, Body>,
    systems: &[(Arc<str>, usize)],
    cancel: Option<&CancelToken>,
    limits: Limits,
    (archive, capture, endpoints): (&Archive, &EntryCapture<'_>, Option<&JumpEndpointRecords>),
    directory: &Path,
) -> Result<(Vec<SystemRead>, PlacementSpools), CatalogError> {
    let mut containers = RowSpoolWriter::new(directory)?;
    let mut locations = RowSpoolWriter::new(directory)?;
    let mut unresolved = RowSpoolWriter::new(directory)?;
    let mut reads = Vec::with_capacity(systems.len());
    for (name, entry) in systems {
        reads.push(System::run_with_output(
            index,
            bodies,
            name,
            *entry,
            cancel,
            limits,
            (
                archive,
                capture,
                endpoints,
                PlacementRows::Spool {
                    containers: &mut containers,
                    locations: &mut locations,
                    unresolved: &mut unresolved,
                },
            ),
        )?);
    }
    let spools = PlacementSpools {
        containers: Some(containers.finish(cancel)?),
        locations: Some(locations.finish(cancel)?),
        unresolved: Some(unresolved.finish(cancel)?),
    };
    Ok((reads, spools))
}
