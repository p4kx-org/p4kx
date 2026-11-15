// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::hash_map::Entry as MapEntry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use p4kx_extraction::{Archive, CancelToken, Guid};
use rayon::prelude::*;

use crate::CatalogError;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::export_ledger::ExportLedger;
use crate::starmap_index::{Index, unreferenced};
use crate::starmap_parse::Read;
use crate::starmap_report::{ContainerCounts, Placement, StarmapReport, apart, clip};
use crate::starmap_walk::{Opened, System, SystemRead};

use jump_class::JumpEndpointRecords;
use jump_endpoint::JumpEndpoint;
use jump_operating::OperatingResolver;

use self::instance::ContainerInstance;
use self::location_instance::LocationInstance;
use self::proof::{PlacementEvidence, PlacementProof};

#[path = "starmap_jump_class.rs"]
pub(crate) mod jump_class;
#[path = "starmap_jump_endpoint.rs"]
pub(crate) mod jump_endpoint;
#[path = "starmap_jump_gate.rs"]
pub(crate) mod jump_gate;
#[path = "starmap_jump_operating.rs"]
pub(crate) mod jump_operating;

#[path = "starmap_container_instance.rs"]
pub(crate) mod instance;

#[path = "starmap_placement_proof.rs"]
pub(crate) mod proof;

#[path = "starmap_location_declaration.rs"]
pub(crate) mod location_declaration;
#[path = "starmap_location_instance.rs"]
pub(crate) mod location_instance;

#[cfg(test)]
#[path = "starmap_installed_witness.rs"]
mod installed_witness;

#[cfg(test)]
#[path = "starmap_installed_jump_endpoints.rs"]
mod installed_jump_endpoints;

#[cfg(test)]
#[path = "starmap_collect_tests.rs"]
mod collect_tests;

/// What bounds the walk through a star system.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    /// The most Child elements one star system may place, itself and its external containers.
    pub(crate) children: usize,
    /// The deepest chain of Child elements that is followed.
    pub(crate) depth: usize,
    /// The deepest chain of external containers that is opened.
    pub(crate) external: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            children: 1 << 22,
            depth: 64,
            external: 32,
        }
    }
}

/// The catalog scope that requires reading system containers.
#[derive(Clone, Copy)]
pub(crate) enum PlacementScope {
    RecordSelection,
    ArchiveInventory,
}

/// The placements of the Starmap Records in the system containers of an Archive.
#[derive(Debug, Default)]
pub(crate) struct Placements {
    pub(crate) ledger: ExportLedger,
    by_record: HashMap<Guid, Placement>,
    ambiguous: HashMap<Guid, Arc<[PlacementProof; 2]>>,
    pub(crate) counts: ContainerCounts,
    pub(crate) report: StarmapReport,
    instances: Vec<ContainerInstance>,
    locations: Vec<LocationInstance>,
    jump_endpoints: Vec<JumpEndpoint>,
    endpoint_inventory_complete: bool,
    instance_availability: Option<DatasetAvailability>,
    /// Every container below the system folder: those the walk opened, each with the record it
    /// hangs below, then those it did not reach, with none.
    pub(crate) opened: Vec<Opened>,
}

impl Placements {
    pub(crate) fn release_record_lookup(&mut self) {
        self.by_record = HashMap::new();
        self.ambiguous = HashMap::new();
    }

    pub(crate) fn take_instances(&mut self) -> CatalogTable<ContainerInstance> {
        CatalogTable {
            availability: self
                .instance_availability
                .unwrap_or(DatasetAvailability::Unavailable),
            rows: std::mem::take(&mut self.instances),
        }
    }

    pub(crate) fn take_locations(&mut self) -> CatalogTable<LocationInstance> {
        CatalogTable {
            availability: self
                .instance_availability
                .unwrap_or(DatasetAvailability::Unavailable),
            rows: std::mem::take(&mut self.locations),
        }
    }

    pub(crate) fn join_jump_operators(
        &mut self,
        entities: &p4kx_assembly::Entities<'_>,
        input: &crate::build::original_input::OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<HashSet<u32>, CatalogError> {
        let mut resolver = OperatingResolver::new(entities, input, cancel);
        self.jump_endpoints = std::mem::take(&mut self.jump_endpoints)
            .into_iter()
            .map(|row| row.with_operating(&mut resolver))
            .collect::<Result<_, _>>()?;
        Ok(resolver.into_selected())
    }

    pub(crate) fn jump_connection_availability(&self) -> DatasetAvailability {
        if self.endpoint_inventory_complete
            && self
                .jump_endpoints
                .iter()
                .all(JumpEndpoint::partner_scope_complete)
        {
            DatasetAvailability::Absent
        } else {
            DatasetAvailability::Unavailable
        }
    }

    pub(crate) fn take_jump_endpoints(&mut self) -> CatalogTable<JumpEndpoint> {
        CatalogTable {
            availability: if self.endpoint_inventory_complete {
                self.instance_availability
                    .unwrap_or(DatasetAvailability::Unavailable)
            } else {
                DatasetAvailability::Unavailable
            },
            rows: std::mem::take(&mut self.jump_endpoints),
        }
    }

    pub(crate) fn get(&self, record: Guid) -> Option<&Placement> {
        self.by_record.get(&record)
    }

    pub(crate) fn ambiguity_proof(&self, record: Guid) -> Option<PlacementEvidence> {
        self.ambiguous
            .get(&record)
            .map(|proof| PlacementEvidence::Ambiguous(Arc::clone(proof)))
    }

    /// Reads the container of every star system of `archive` and the external containers it
    /// names, each container once, then composes each Child's position with the positions and
    /// rotations of the Children above it. A record placed more than once keeps its first
    /// placement in document order, the systems taken in the order of their folder names; one
    /// that more than one Child places at places more than `SAME_PLACE` metres apart has no
    /// position.
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
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            Limits::default(),
            capture,
            scope,
            Some(endpoints),
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
        Self::read_scoped(archive, cancel, limits, capture, scope, None)
    }

    fn read_scoped(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        scope: PlacementScope,
        endpoints: Option<&JumpEndpointRecords>,
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
        let read = systems
            .par_iter()
            .map(|(name, entry)| {
                System::run(
                    &index,
                    &bodies,
                    name,
                    *entry,
                    cancel,
                    limits,
                    (archive, capture, endpoints),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        drop(bodies);
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let mut placements = Self {
            ledger,
            ..Self::default()
        };
        let endpoint_reads_complete = !placements
            .ledger
            .incomplete_inventories
            .contains(&crate::Kind::StarmapObjects);
        placements.finish(&index, systems.len(), read, scope);
        placements.endpoint_inventory_complete = endpoint_reads_complete
            && endpoints.is_some_and(|records| records.complete)
            && placements.counts.unreadable == 0
            && placements.counts.missing == 0;
        placements.report.elapsed = started.elapsed();
        Ok(placements)
    }

    fn finish(
        &mut self,
        index: &Index<'_>,
        roots: usize,
        read: Vec<SystemRead>,
        scope: PlacementScope,
    ) {
        self.counts.systems = roots;
        self.instance_availability = Some(if roots == 0 {
            DatasetAvailability::Absent
        } else {
            DatasetAvailability::Available
        });
        let mut opened = HashSet::new();
        let named = self.merge_systems(read, &mut opened, scope);
        self.counts.ambiguous = self.ambiguous.len();
        self.report
            .containers
            .push(format!("{named} containers name starmap records"));
        for record in self.ambiguous.keys() {
            self.by_record.remove(record);
        }
        self.report.unreferenced = unreferenced(index, &opened);
        if matches!(scope, PlacementScope::RecordSelection) {
            self.add_unreached(index, &opened);
        }
    }

    fn merge_systems(
        &mut self,
        systems: Vec<SystemRead>,
        opened: &mut HashSet<usize>,
        scope: PlacementScope,
    ) -> usize {
        let mut named = 0;
        for system in systems {
            opened.extend(system.containers.iter().map(|container| container.entry));
            self.report.deepest_external = self.report.deepest_external.max(system.deepest);
            named += system.named;
            self.counts.unreadable += system.unreadable;
            self.counts.missing += system.missing;
            self.counts.malformed += system.malformed;
            self.report.containers.extend(system.report.containers);
            self.report.issues.extend(system.report.issues);
            if matches!(scope, PlacementScope::RecordSelection) {
                append_owned(&mut self.opened, system.containers);
            }
            append_owned(&mut self.instances, system.instances);
            append_owned(&mut self.locations, system.locations);
            append_owned(&mut self.jump_endpoints, system.jump_endpoints);
            for (record, placement) in system.placed {
                self.add(record, placement);
            }
        }
        named
    }

    /// Adds the containers below the system folder that the walk did not open, each hanging below
    /// no record.
    fn add_unreached(&mut self, index: &Index<'_>, opened: &HashSet<usize>) {
        self.opened.extend(
            index
                .system_containers()
                .into_iter()
                .filter(|(_, entry)| !opened.contains(entry))
                .map(|(system, entry)| Opened {
                    system,
                    entry,
                    under: None,
                }),
        );
    }

    /// Retains the first placement and the first pair establishing an ambiguous position.
    fn add(&mut self, record: Guid, placement: Placement) {
        let MapEntry::Occupied(held) = self.by_record.entry(record) else {
            self.by_record.insert(record, placement);
            return;
        };
        let moved = apart(held.get().position, placement.position);
        if moved {
            self.ambiguous
                .entry(record)
                .or_insert_with(|| Arc::new([held.get().proof.clone(), placement.proof.clone()]));
        }
        if self.report.repeated.len() < 100 {
            self.report.repeated.push(format!(
                "{record}: {} {}/{} {:?}, also {}/{} {:?}",
                if moved {
                    "left unplaced:"
                } else {
                    "same place:"
                },
                held.get().system,
                clip(&held.get().entity_name),
                held.get().position,
                placement.system,
                clip(&placement.entity_name),
                placement.position
            ));
        }
    }
}

fn append_owned<T>(rows: &mut Vec<T>, added: Vec<T>) {
    if rows.is_empty() {
        *rows = added;
    } else {
        rows.extend(added);
    }
}
