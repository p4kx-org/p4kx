// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The walk through the containers that entities of containers place.

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{Archive, CancelToken, FieldOrigin, Guid};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

use crate::CatalogError;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_rows::staged::{IndexedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;
use crate::export_ledger::ExportLedger;
use crate::loot_entxml::documents::LootEntityContainerDocument;
use crate::loot_unreadable::Unreadable;
use crate::starmap_index::Index;
use crate::starmap_walk::Opened;

#[path = "loot_walk_container.rs"]
mod container;

use container::ContainerRead;

/// An entity of a container that carries a multi config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Placed {
    pub(crate) config: Guid,
    pub(crate) config_origin: FieldOrigin,
    /// The path of the placed container: the Archive's when it has it, else as written.
    pub(crate) path: String,
    pub(crate) path_origin: FieldOrigin,
    pub(crate) target: PlacedTarget,
}

/// Resolution of the container selected by an original entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlacedTarget {
    Resolved(usize),
    Missing,
    VerifiedMissing,
    Ambiguous,
}

impl PlacedTarget {
    pub(crate) const fn entry(self) -> Option<usize> {
        match self {
            Self::Resolved(entry) => Some(entry),
            Self::Missing | Self::VerifiedMissing | Self::Ambiguous => None,
        }
    }
}

/// What the entities of one container say: the containers they place and the configs they carry.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Summary {
    pub(crate) nested: Vec<usize>,
    pub(crate) placed: Vec<Placed>,
    pub(crate) unreadable: Option<Unreadable>,
    /// The entity files that name several different containers or configs, of which only the
    /// first of each is used.
    pub(crate) several: Vec<String>,
}

/// The containers read, by physical Archive Entry, with their source observations.
pub(crate) struct Explored {
    pub(crate) ledger: ExportLedger,
    pub(crate) summaries: HashMap<usize, Summary>,
    pub(crate) original_documents: StagedCatalogTable,
}

/// Reads the containers in `opened` and every container their entities place, breadth first.
/// Each physical Entry is read once.
pub(crate) fn explore(
    archive: &Archive,
    index: &Index<'_>,
    opened: &[Opened],
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<Explored, CatalogError> {
    let scratch = std::env::temp_dir();
    let mut originals = IndexedCatalogRows::new(&scratch, cancel);
    let mut available = false;
    let walked = visit_documents(
        archive,
        index,
        opened,
        cancel,
        capture,
        None,
        |entry, document| {
            originals.append((entry, document.physical_index()?), &document)?;
            available = true;
            Ok(())
        },
    )?;
    let availability = if available {
        DatasetAvailability::Available
    } else if !walked.documents_complete {
        DatasetAvailability::Unavailable
    } else {
        DatasetAvailability::Absent
    };
    Ok(Explored {
        ledger: walked.ledger,
        summaries: walked.summaries,
        original_documents: originals.finish_with_availability(availability)?,
    })
}

/// One selected containing Entry and optional original nested Entry index.
#[derive(Clone, Copy)]
pub(crate) struct SelectedDocument {
    pub(crate) container_position: usize,
    pub(crate) entity_index: Option<usize>,
}

pub(crate) struct Walked {
    pub(crate) ledger: ExportLedger,
    pub(crate) summaries: HashMap<usize, Summary>,
    pub(crate) documents_complete: bool,
    pub(crate) routes_complete: bool,
    pub(crate) found_owner: bool,
}

pub(crate) fn visit_documents(
    archive: &Archive,
    index: &Index<'_>,
    opened: &[Opened],
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    selected: Option<SelectedDocument>,
    mut visit: impl FnMut(usize, LootEntityContainerDocument) -> Result<(), CatalogError>,
) -> Result<Walked, CatalogError> {
    let directory = capture
        .source_uses()
        .observe_root_directory_paths(archive)?;
    let inventory_complete = crate::export_archive_health::inventory(archive).is_none();
    let mut walked = Walked {
        ledger: ExportLedger::default(),
        summaries: HashMap::new(),
        documents_complete: inventory_complete,
        routes_complete: inventory_complete,
        found_owner: false,
    };
    let mut frontier: Vec<usize> = opened.iter().map(|root| root.entry).collect();
    let read = |position, member| {
        container::summarise(
            archive,
            index,
            position,
            cancel,
            capture,
            directory.as_ref(),
            member,
        )
    };
    while !frontier.is_empty() {
        crate::catalog_query_session::checkpoint(cancel)?;
        frontier.sort_unstable();
        frontier.dedup();
        frontier.retain(|entry| !walked.summaries.contains_key(entry));
        if let Some(wanted) = selected
            && frontier.binary_search(&wanted.container_position).is_ok()
        {
            let selected_read = read(wanted.container_position, wanted.entity_index)?;
            walked.found_owner = true;
            walked.documents_complete = !selected_read.scope_unknown;
            walked.merge(wanted.container_position, selected_read, &mut visit)?;
            crate::catalog_query_session::checkpoint(cancel)?;
            return Ok(walked);
        }
        let layer: Vec<_> = frontier
            .par_iter()
            .map(|&entry| {
                let mut outcome = read(entry, None)?;
                if selected.is_some() {
                    outcome.documents = Vec::new();
                }
                Ok((entry, outcome))
            })
            .collect::<Result<Vec<_>, CatalogError>>()?;
        frontier.clear();
        for (entry, outcome) in layer {
            frontier.extend(outcome.summary.nested.iter().copied());
            walked.merge(entry, outcome, &mut visit)?;
        }
    }
    crate::catalog_query_session::checkpoint(cancel)?;
    Ok(walked)
}

impl Walked {
    fn merge(
        &mut self,
        entry: usize,
        read: ContainerRead,
        visit: &mut impl FnMut(usize, LootEntityContainerDocument) -> Result<(), CatalogError>,
    ) -> Result<(), CatalogError> {
        for document in read.documents {
            visit(entry, document)?;
        }
        self.documents_complete &= !read.scope_unknown;
        self.routes_complete &= !read.route_unknown && !read.scope_unknown;
        self.summaries.insert(entry, read.summary);
        self.ledger.merge(read.ledger);
        Ok(())
    }
}

/// Returns every [`Placed`] in `root` and the containers below it, each container once; a cycle
/// of containers ends where it returns to a container already seen. The cost is the size of the
/// subgraph below `root`, once per distinct root.
pub(crate) fn below(root: usize, summaries: &HashMap<usize, Summary>) -> Vec<&Placed> {
    let mut seen: HashSet<usize> = HashSet::from([root]);
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(entry) = pending.pop() {
        let Some(summary) = summaries.get(&entry) else {
            continue;
        };
        found.extend(&summary.placed);
        pending.extend(summary.nested.iter().filter(|&&next| seen.insert(next)));
    }
    found
}

/// Returns the path of the Archive entry `entry`, empty for none.
pub(crate) fn path_of(archive: &Archive, entry: usize) -> &str {
    archive
        .entries()
        .get(entry)
        .map_or("", p4kx_extraction::Entry::path)
}

#[cfg(test)]
#[path = "loot_walk_tests.rs"]
mod tests;
