// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use p4kx_extraction::{Archive, CancelToken};
#[cfg(test)]
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

use super::{JumpEndpointRecords, Limits, PlacementSpools, PlacementStorage};
use crate::CatalogError;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_rows::spool::RowSpoolWriter;
use crate::starmap_index::Index;
use crate::starmap_parse::Body;
use crate::starmap_walk::{PlacementRows, System, SystemRead};

pub(super) fn walk_systems(
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
        PlacementStorage::Harvest(harvest) => walk_descriptor_systems(
            index,
            bodies,
            systems,
            cancel,
            limits,
            (archive, capture),
            || PlacementRows::Harvest(harvest),
        ),
        PlacementStorage::OpenedContainers => walk_descriptor_systems(
            index,
            bodies,
            systems,
            cancel,
            limits,
            (archive, capture),
            || PlacementRows::OpenedContainers,
        ),
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

fn walk_descriptor_systems<'a>(
    index: &'a Index<'a>,
    bodies: &'a HashMap<usize, Body>,
    systems: &[(Arc<str>, usize)],
    cancel: Option<&'a CancelToken>,
    limits: Limits,
    (archive, capture): (&'a Archive, &'a EntryCapture<'a>),
    rows: impl Fn() -> PlacementRows<'a>,
) -> Result<(Vec<SystemRead>, PlacementSpools), CatalogError> {
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
                (archive, capture, None, rows()),
            )
        })
        .collect::<Result<_, _>>()?;
    Ok((reads, PlacementSpools::default()))
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
