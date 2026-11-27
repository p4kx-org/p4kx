// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use p4kx_extraction::{Archive, CancelToken};
#[cfg(test)]
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

use super::super::name_coverage;
use super::{JumpEndpointRecords, Limits, PlacementSpools, PlacementStorage, Placements};
use crate::CatalogError;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_rows::spool::RowSpoolWriter;
use crate::entry_source;
use crate::export_ledger::ExportLedger;
use crate::starmap_index::{Index, SystemRootCandidates};
use crate::starmap_parse::{self, Body, DescriptorScope, DescriptorSurvey, Read};
use crate::starmap_walk::{ObjectSelection, PlacementRows, System, SystemRead};
use crate::{ExportAccountingError, Kind};

pub(super) struct SystemDescriptors {
    pub(super) systems: Vec<(Arc<str>, usize)>,
    pub(super) system_count: usize,
    pub(super) all_roots: bool,
    pub(super) names_complete: bool,
    pub(super) read: Read,
}

pub(super) fn read_descriptors(
    archive: &Archive,
    index: &Index<'_>,
    roots: Vec<(Arc<str>, usize)>,
    cancel: Option<&CancelToken>,
    bounds: (Option<usize>, DescriptorScope),
    capture: &EntryCapture<'_>,
) -> Result<SystemDescriptors, CatalogError> {
    let system_count = roots.len();
    let mut ledger = ExportLedger::default();
    let systems = unique_system_roots(archive, index, capture, roots, &mut ledger)?;
    let root_entries: Vec<usize> = systems.iter().map(|(_, entry)| *entry).collect();
    let all_roots = root_entries.len() == system_count;
    let names_complete = all_roots && name_coverage::descriptor_inputs_complete(&ledger);
    let mut read =
        starmap_parse::read_all_scoped(archive, index, &root_entries, cancel, bounds, capture)?;
    read.ledger.merge(ledger);
    Ok(SystemDescriptors {
        systems,
        system_count,
        all_roots,
        names_complete: names_complete && read.name_inputs_complete,
        read,
    })
}

pub(super) fn walk_systems(
    index: &Index<'_>,
    bodies: &HashMap<usize, Body>,
    systems: &[(Arc<str>, usize)],
    cancel: Option<&CancelToken>,
    limits: Limits,
    (archive, capture, endpoints): (&Archive, &EntryCapture<'_>, Option<&JumpEndpointRecords>),
    storage: PlacementStorage<'_>,
) -> Result<(Vec<SystemRead>, PlacementSpools), CatalogError> {
    let descriptors = DescriptorWalk {
        index,
        bodies,
        systems,
        cancel,
        limits,
    };
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
            descriptors.walk((archive, capture, None), || PlacementRows::Harvest(harvest))
        }
        PlacementStorage::OpenedContainers | PlacementStorage::JumpEndpoints(None) => descriptors
            .walk((archive, capture, endpoints), || {
                PlacementRows::OpenedContainers
            }),
        PlacementStorage::JumpEndpoints(Some(records)) => descriptors
            .walk((archive, capture, endpoints), || {
                PlacementRows::Object(ObjectSelection::Records(records))
            }),
        PlacementStorage::ShopNames(guids) => {
            descriptors.walk((archive, capture, None), || PlacementRows::ShopNames(guids))
        }
        PlacementStorage::Object(selection) => descriptors.walk((archive, capture, None), || {
            PlacementRows::Object(selection)
        }),
        #[cfg(test)]
        PlacementStorage::Typed => walk_typed_systems(
            index,
            bodies,
            systems,
            cancel,
            limits,
            (archive, capture, endpoints),
        ),
    }
}

#[cfg(test)]
fn walk_typed_systems(
    index: &Index<'_>,
    bodies: &HashMap<usize, Body>,
    systems: &[(Arc<str>, usize)],
    cancel: Option<&CancelToken>,
    limits: Limits,
    (archive, capture, endpoints): (&Archive, &EntryCapture<'_>, Option<&JumpEndpointRecords>),
) -> Result<(Vec<SystemRead>, PlacementSpools), CatalogError> {
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

struct DescriptorWalk<'read, 'roots> {
    index: &'read Index<'read>,
    bodies: &'read HashMap<usize, Body>,
    systems: &'roots [(Arc<str>, usize)],
    cancel: Option<&'read CancelToken>,
    limits: Limits,
}

impl<'read> DescriptorWalk<'read, '_> {
    fn walk(
        &self,
        (archive, capture, endpoints): (
            &'read Archive,
            &'read EntryCapture<'read>,
            Option<&'read JumpEndpointRecords>,
        ),
        rows: impl Fn() -> PlacementRows<'read>,
    ) -> Result<(Vec<SystemRead>, PlacementSpools), CatalogError> {
        let reads = self
            .systems
            .iter()
            .map(|(name, entry)| {
                System::run_with_output(
                    self.index,
                    self.bodies,
                    name,
                    *entry,
                    self.cancel,
                    self.limits,
                    (archive, capture, endpoints, rows()),
                )
            })
            .collect::<Result<_, _>>()?;
        Ok((reads, PlacementSpools::default()))
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
    if systems.is_empty() {
        return Ok((Vec::new(), PlacementSpools::default()));
    }
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
    let grid = crate::location_physics::stage::write(&reads, directory, cancel)?;
    let spools = PlacementSpools {
        containers: Some(containers.finish(cancel)?),
        locations: Some(locations.finish(cancel)?),
        unresolved: Some(unresolved.finish(cancel)?),
        location_physics_grids: grid.spool,
        location_physics_availability: Some(grid.availability),
    };
    Ok((reads, spools))
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
        let candidates = index
            .system_root_candidates(index_entry)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        match candidates {
            SystemRootCandidates::Unique(entry) => unique.push((name, *entry)),
            SystemRootCandidates::Ambiguous(candidates) => {
                entry_source::record_ambiguous_outer_entries(
                    archive,
                    capture,
                    ledger,
                    Kind::StarmapObjects,
                    None,
                    candidates,
                )?;
            }
        }
    }
    Ok(unique)
}

impl Placements {
    pub(super) fn finish_scope_coverage(
        &mut self,
        capture: &EntryCapture<'_>,
        endpoints: Option<&JumpEndpointRecords>,
        routes_complete: bool,
        survey: Option<DescriptorSurvey>,
    ) {
        self.endpoint_inventory_complete = self.name_coverage.directory_complete
            && self.has_complete_endpoints(routes_complete, endpoints);
        self.loot_survey = survey.map(|survey| {
            survey.finish(
                capture.source_uses(),
                routes_complete && self.loot_seed_routes_complete(),
            )
        });
    }

    fn loot_seed_routes_complete(&self) -> bool {
        self.name_coverage.directory_complete
            && self.name_coverage.descriptors_complete
            && !self.placement_routes_incomplete
            && self.counts.unreadable == 0
            && self.counts.missing == self.counts.verified_missing
            && self.counts.malformed == 0
            && !self
                .ledger
                .incomplete_inventories
                .contains(&Kind::StarmapObjects)
    }
}
