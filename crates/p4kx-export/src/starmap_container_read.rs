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
use super::{Limits, PlacementScope, Placements};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_rows::spool::{RowSpool, RowSpoolWriter};
use crate::catalog_table::DatasetAvailability;
use crate::starmap_index::Index;
use crate::starmap_parse::{Body, Read};
use crate::starmap_walk::{ContainerRows, System, SystemRead};
use crate::{CatalogError, Kind};

#[derive(Clone, Copy)]
enum InstanceStorage<'directory> {
    Spool(&'directory Path),
    #[cfg(test)]
    Typed,
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
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
        endpoints: &JumpEndpointRecords,
        directory: &Path,
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            Limits::default(),
            capture,
            scope,
            Some(endpoints),
            InstanceStorage::Spool(directory),
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
            InstanceStorage::Typed,
        )
    }

    fn read_scoped(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
        endpoints: Option<&JumpEndpointRecords>,
        storage: InstanceStorage<'_>,
    ) -> Result<Self, CatalogError> {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
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
        let roots: Vec<usize> = systems.iter().map(|(_, entry)| *entry).collect();
        let Read { bodies, ledger, .. } = crate::starmap_parse::read_all_with_jumps(
            archive,
            &index,
            &roots,
            cancel,
            limits.external,
            (limits.depth, limits.children, endpoints.is_some()),
            capture,
        )?;
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let (read, instance_spool) = walk_systems(
            &index,
            &bodies,
            &systems,
            cancel,
            limits,
            (archive, capture, endpoints),
            storage,
        )?;
        drop(bodies);
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let endpoint_scope_complete = !ledger
            .incomplete_inventories
            .contains(&Kind::StarmapObjects)
            && read.iter().all(|system| !system.routes_incomplete);
        let mut placements = Self {
            ledger,
            instance_spool,
            ..Self::default()
        };
        placements.finish(&index, systems.len(), read, scope);
        placements.endpoint_inventory_complete = endpoint_scope_complete
            && endpoints.is_some_and(|records| records.complete)
            && placements.counts.unreadable == 0
            && placements.counts.missing == 0;
        placements.report.elapsed = started.elapsed();
        Ok(placements)
    }
}

fn walk_systems(
    index: &Index<'_>,
    bodies: &HashMap<usize, Body>,
    systems: &[(Arc<str>, usize)],
    cancel: Option<&CancelToken>,
    limits: Limits,
    (archive, capture, endpoints): (&Archive, &EntryCapture<'_>, Option<&JumpEndpointRecords>),
    storage: InstanceStorage<'_>,
) -> Result<(Vec<SystemRead>, Option<RowSpool>), CatalogError> {
    if systems.is_empty() {
        return Ok((Vec::new(), None));
    }
    match storage {
        InstanceStorage::Spool(directory) => {
            let mut writer = RowSpoolWriter::new(directory)?;
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
                        ContainerRows::Spool(&mut writer),
                    ),
                )?);
            }
            Ok((reads, Some(writer.finish(cancel)?)))
        }
        #[cfg(test)]
        InstanceStorage::Typed => {
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
            Ok((reads, None))
        }
    }
}
