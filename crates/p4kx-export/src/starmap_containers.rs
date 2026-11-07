// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::hash_map::Entry as MapEntry;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use p4kx_extraction::{Archive, CancelToken, Guid};
use rayon::prelude::*;

use crate::CatalogError;
use crate::export_ledger::ExportLedger;
use crate::starmap_index::{Index, unreferenced};
use crate::starmap_parse::{Read, read_all};
use crate::starmap_report::{ContainerCounts, Placement, StarmapReport, apart, clip};
use crate::starmap_walk::{Opened, System, SystemRead};

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

/// The placements of the Starmap Records in the system containers of an Archive.
#[derive(Debug, Default)]
pub(crate) struct Placements {
    pub(crate) ledger: ExportLedger,
    by_record: HashMap<Guid, Placement>,
    pub(crate) counts: ContainerCounts,
    pub(crate) report: StarmapReport,
    /// Every container below the system folder: those the walk opened, each with the record it
    /// hangs below, then those it did not reach, with none.
    pub(crate) opened: Vec<Opened>,
}

impl Placements {
    pub(crate) fn get(&self, record: Guid) -> Option<&Placement> {
        self.by_record.get(&record)
    }

    /// Reads the container of every star system of `archive` and the external containers it
    /// names, each container once, then composes each Child's position with the positions and
    /// rotations of the Children above it. A record placed more than once keeps its first
    /// placement in document order, the systems taken in the order of their folder names; one
    /// that more than one Child places at places more than `SAME_PLACE` metres apart has no
    /// position.
    pub(crate) fn read(
        archive: &Archive,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        Self::read_within(archive, cancel, Limits::default())
    }

    pub(crate) fn read_within(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
    ) -> Result<Self, CatalogError> {
        let started = Instant::now();
        let index = Index::new(archive);
        let systems = index.systems();
        let roots: Vec<usize> = systems.iter().map(|(_, entry)| *entry).collect();
        let Read { bodies, ledger, .. } = read_all(
            archive,
            &index,
            &roots,
            cancel,
            limits.external,
            (limits.depth, limits.children),
        );
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let read: Vec<SystemRead> = systems
            .par_iter()
            .map(|(name, entry)| System::run(&index, &bodies, name, *entry, cancel, limits))
            .collect();
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let mut placements = Self {
            ledger,
            ..Self::default()
        };
        placements.counts.systems = systems.len();
        let mut repeated: HashSet<Guid> = HashSet::new();
        let mut opened: HashSet<usize> = HashSet::new();
        let mut named = 0;
        for system in read {
            opened.extend(system.opened.iter().copied());
            placements.report.deepest_external =
                placements.report.deepest_external.max(system.deepest);
            named += system.named;
            placements.counts.unreadable += system.unreadable;
            placements.counts.missing += system.missing;
            placements.counts.malformed += system.malformed;
            placements
                .report
                .containers
                .extend(system.report.containers);
            placements.report.issues.extend(system.report.issues);
            placements.opened.extend(system.containers);
            for (record, placement) in system.placed {
                placements.add(record, placement, &mut repeated);
            }
        }
        placements.counts.ambiguous = repeated.len();
        placements
            .report
            .containers
            .push(format!("{named} containers name starmap records"));
        for record in &repeated {
            placements.by_record.remove(record);
        }
        placements.report.unreferenced = unreferenced(&index, &opened);
        placements.add_unreached(&index, &opened);
        placements.report.elapsed = started.elapsed();
        Ok(placements)
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

    /// Keeps the first placement of `record`; a later one more than `SAME_PLACE` metres from it
    /// puts the record into `repeated`, to be left unplaced.
    fn add(&mut self, record: Guid, placement: Placement, repeated: &mut HashSet<Guid>) {
        let MapEntry::Occupied(held) = self.by_record.entry(record) else {
            self.by_record.insert(record, placement);
            return;
        };
        let moved = apart(held.get().position, placement.position);
        if moved {
            repeated.insert(record);
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
