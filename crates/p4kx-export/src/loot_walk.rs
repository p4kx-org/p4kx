// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The walk through the containers that entities of containers place.

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::ops::ControlFlow;

use p4kx_extraction::{Archive, CancelToken, FieldOrigin, Guid, SourceUse, SourceUses};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_rows::staged::{IndexedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;
use crate::export_ledger::ExportLedger;
use crate::loot_container_source::EntXmlAuthor;
use crate::loot_entxml::documents::LootEntityContainerDocument;
use crate::loot_unreadable::Unreadable;
use crate::starmap_index::Index;
use crate::starmap_walk::Opened;
use crate::{CatalogError, ExportAccountingError};

#[path = "loot_walk_container.rs"]
mod container;

use container::{ContainerRead, DocumentOutput};

/// An entity of a container that carries a multi config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Placed {
    pub(crate) author: EntXmlAuthor,
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
        opened.iter().map(|seed| seed.entry),
        cancel,
        capture,
        Some(DocumentVisit {
            selected: None,
            visit: |entry, document: LootEntityContainerDocument| {
                originals.append((entry, document.physical_index()?), &document)?;
                available = true;
                Ok(())
            },
        }),
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

/// Emits all reached documents or those of one exact reached physical owner.
pub(crate) struct DocumentVisit<Visit> {
    pub(crate) selected: Option<SelectedDocument>,
    pub(crate) visit: Visit,
}

pub(crate) struct Walked {
    pub(crate) ledger: ExportLedger,
    pub(crate) summaries: HashMap<usize, Summary>,
    pub(crate) documents_complete: bool,
    pub(crate) routes_complete: bool,
    pub(crate) found_owner: bool,
    /// Original directory and verified documents examined by an exhaustive link lookup.
    pub(crate) document_census: Option<FieldOrigin>,
}

/// Walks physically seeded `EntXML` links, optionally delivering document rows.
pub(crate) fn visit_documents(
    archive: &Archive,
    index: &Index<'_>,
    seeds: impl Iterator<Item = usize>,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    mut documents: Option<
        DocumentVisit<impl FnMut(usize, LootEntityContainerDocument) -> Result<(), CatalogError>>,
    >,
) -> Result<Walked, CatalogError> {
    let directory = capture
        .source_uses()
        .observe_root_directory_paths(archive)?;
    let mut walked = Walked::new(archive);
    let mut frontier: Vec<usize> = seeds.collect();
    let mut layer = Vec::new();
    let read = |position, output| {
        container::summarise(
            archive,
            index,
            position,
            cancel,
            capture,
            directory.as_ref(),
            output,
        )
    };
    let selected = documents.as_ref().and_then(|documents| documents.selected);
    let mut census = documents
        .is_none()
        .then(|| directory.iter().cloned().collect());
    let frontier_output = match (&census, selected) {
        (Some(_), _) => DocumentOutput::LinkCensus,
        (None, Some(_)) => DocumentOutput::LinksOnly,
        (None, None) => DocumentOutput::Keep(None),
    };
    while !frontier.is_empty() {
        checkpoint(cancel)?;
        frontier.sort_unstable();
        frontier.dedup();
        frontier.retain(|entry| !walked.summaries.contains_key(entry));
        if let Some(wanted) = selected
            && frontier.binary_search(&wanted.container_position).is_ok()
        {
            let selected_read = read(
                wanted.container_position,
                DocumentOutput::Keep(wanted.entity_index),
            )?;
            walked.finish_selected(wanted.container_position, selected_read, &mut documents)?;
            checkpoint(cancel)?;
            return Ok(walked);
        }
        std::mem::swap(&mut frontier, &mut layer);
        walked.visit_layer(
            &layer,
            &mut frontier,
            |entry| read(entry, frontier_output),
            &mut documents,
            &mut census,
            cancel,
        )?;
        layer.clear();
    }
    checkpoint(cancel)?;
    if let Some(census) = census {
        walked.finish_census(census, directory.is_some(), capture.source_uses());
    }
    Ok(walked)
}

impl Walked {
    fn new(archive: &Archive) -> Self {
        let complete = crate::export_archive_health::inventory(archive).is_none();
        Self {
            ledger: ExportLedger::default(),
            summaries: HashMap::new(),
            documents_complete: complete,
            routes_complete: complete,
            found_owner: false,
            document_census: None,
        }
    }

    fn finish_selected(
        &mut self,
        entry: usize,
        read: ContainerRead,
        documents: &mut Option<
            DocumentVisit<
                impl FnMut(usize, LootEntityContainerDocument) -> Result<(), CatalogError>,
            >,
        >,
    ) -> Result<(), CatalogError> {
        self.found_owner = true;
        self.documents_complete = !read.scope_unknown;
        self.merge(entry, read, documents)
    }

    fn visit_layer(
        &mut self,
        layer: &[usize],
        frontier: &mut Vec<usize>,
        read: impl Fn(usize) -> Result<ContainerRead, CatalogError> + Sync,
        documents: &mut Option<
            DocumentVisit<
                impl FnMut(usize, LootEntityContainerDocument) -> Result<(), CatalogError>,
            >,
        >,
        census: &mut Option<Vec<SourceUse>>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let batch_width = if census.is_some() {
            rayon::current_num_threads()
        } else {
            layer.len().max(1)
        };
        for entries in layer.chunks(batch_width) {
            checkpoint(cancel)?;
            let batch: Vec<_> = entries
                .par_iter()
                .map(|&entry| read(entry).map(|read| (entry, read)))
                .collect::<Result<_, _>>()?;
            for (entry, mut read) in batch {
                frontier.extend(read.summary.nested.iter().copied());
                if let Some(census) = census {
                    census.extend(
                        read.census
                            .take()
                            .ok_or(ExportAccountingError::ConflictingSourceDatum)?,
                    );
                }
                self.merge(entry, read, documents)?;
            }
        }
        Ok(())
    }

    fn finish_census(&mut self, mut census: Vec<SourceUse>, directory: bool, sources: &SourceUses) {
        census.sort_unstable_by(|left, right| left.reference().cmp(right.reference()));
        census.dedup_by(|left, right| left.reference() == right.reference());
        self.document_census = Some(sources.origin(&census, directory && self.routes_complete));
        self.routes_complete = self
            .document_census
            .as_ref()
            .is_some_and(FieldOrigin::source_set_complete);
    }

    fn merge(
        &mut self,
        entry: usize,
        read: ContainerRead,
        documents: &mut Option<
            DocumentVisit<
                impl FnMut(usize, LootEntityContainerDocument) -> Result<(), CatalogError>,
            >,
        >,
    ) -> Result<(), CatalogError> {
        for document in read.documents {
            let visitor = documents
                .as_mut()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            (visitor.visit)(entry, document)?;
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
    let mut found = Vec::new();
    let _: ControlFlow<Infallible> = visit_below(root, summaries, |placed| {
        found.push(placed);
        ControlFlow::Continue(())
    });
    found
}

/// Visits original placements in writer order until the caller stops.
pub(crate) fn visit_below<'source, Match>(
    root: usize,
    summaries: &'source HashMap<usize, Summary>,
    mut visit: impl FnMut(&'source Placed) -> ControlFlow<Match>,
) -> ControlFlow<Match> {
    let mut seen: HashSet<usize> = HashSet::from([root]);
    let mut pending = vec![root];
    while let Some(entry) = pending.pop() {
        let Some(summary) = summaries.get(&entry) else {
            continue;
        };
        for placed in &summary.placed {
            visit(placed)?;
        }
        pending.extend(summary.nested.iter().filter(|&&next| seen.insert(next)));
    }
    ControlFlow::Continue(())
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
