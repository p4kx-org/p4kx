// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The walk through the containers that entities of containers place.

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{Archive, CancelToken, Guid, SourceUse};
use rayon::prelude::*;

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture};
use crate::catalog_rows::staged::{IndexedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;
use crate::entry_source::{self, EntrySource};
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::loot_entxml::documents::LootEntityContainerDocument;
use crate::loot_entxml::{CapturedEntityLink, EntityLink};
use crate::loot_unreadable::Unreadable;
use crate::starmap_index::Index;
use crate::starmap_walk::Opened;
use crate::{CatalogError, ExportAccountingError, Kind};

/// An entity of a container that carries a multi config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Placed {
    pub(crate) config: Guid,
    /// The path of the placed container: the Archive's when it has it, else as written.
    pub(crate) path: String,
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
    let mut ledger = ExportLedger::default();
    let directory = capture
        .source_uses()
        .observe_root_directory_paths(archive)?;
    let scratch = std::env::temp_dir();
    let mut originals = IndexedCatalogRows::new(&scratch, cancel);
    let mut available = false;
    let mut unknown = crate::export_archive_health::inventory(archive).is_some();
    let mut summaries: HashMap<usize, Summary> = HashMap::new();
    let mut frontier: Vec<usize> = opened.iter().map(|root| root.entry).collect();
    while !frontier.is_empty() && !cancel.is_some_and(CancelToken::is_cancelled) {
        frontier.sort_unstable();
        frontier.dedup();
        frontier.retain(|entry| !summaries.contains_key(entry));
        let read: Vec<_> = frontier
            .par_iter()
            .filter(|_| !cancel.is_some_and(CancelToken::is_cancelled))
            .map(|&entry| {
                summarise(archive, index, entry, cancel, capture, directory.as_ref())
                    .map(|read| (entry, read))
            })
            .collect::<Result<Vec<_>, _>>()?;
        frontier = read
            .iter()
            .flat_map(|(_, read)| read.summary.nested.iter().copied())
            .collect();
        for (entry, read) in read {
            for document in read.documents {
                originals.append((entry, document.physical_index()?), &document)?;
                available = true;
            }
            unknown |= read.scope_unknown;
            summaries.insert(entry, read.summary);
            ledger.merge(read.ledger);
        }
    }
    let availability = if available {
        DatasetAvailability::Available
    } else if unknown {
        DatasetAvailability::Unavailable
    } else {
        DatasetAvailability::Absent
    };
    Ok(Explored {
        ledger,
        summaries,
        original_documents: originals.finish_with_availability(availability)?,
    })
}

struct ContainerRead {
    summary: Summary,
    ledger: ExportLedger,
    documents: Vec<LootEntityContainerDocument>,
    scope_unknown: bool,
}

fn summarise(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    directory: Option<&SourceUse>,
) -> Result<ContainerRead, CatalogError> {
    let read = crate::loot_container_source::read(archive, entry, cancel, capture)?;
    let mut ledger = read.ledger;
    let scope_unknown = read.unreadable.is_some() || read.original_scope_unknown;
    let mut documents = Vec::new();
    let mut summary = Summary {
        unreadable: read.unreadable,
        ..Summary::default()
    };
    for (path, mut link) in read.links {
        let absence_proved = retain_original(
            &mut link,
            archive,
            index,
            directory,
            capture,
            cancel,
            &mut documents,
        )?;
        let (link, selected) = link.into_parts();
        let EntityLink {
            container: written,
            config,
            several,
            malformed_config: _,
        } = link;
        if several {
            summary.several.push(path);
        }
        let target = if let Some(path) = written.as_deref() {
            let selector = selected.ok_or(ExportAccountingError::UnboundHealthSubject)?;
            if selector.as_str() != path {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            resolve_target(
                archive,
                index,
                capture,
                &mut ledger,
                selector,
                absence_proved,
            )?
        } else {
            PlacedTarget::Missing
        };
        summary.add(archive, config, written, target);
    }
    summary.nested.sort_unstable();
    summary.nested.dedup();
    Ok(ContainerRead {
        summary,
        ledger,
        documents,
        scope_unknown,
    })
}

fn retain_original(
    link: &mut CapturedEntityLink,
    archive: &Archive,
    index: &Index<'_>,
    directory: Option<&SourceUse>,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    documents: &mut Vec<LootEntityContainerDocument>,
) -> Result<bool, CatalogError> {
    let Some(original) = link.take_original() else {
        return Ok(false);
    };
    let selected = if link.value().several || link.value().malformed_config {
        None
    } else {
        link.selected_container()
    };
    let projected = original.project(archive, index, directory, capture, cancel, selected)?;
    documents.push(projected.document);
    Ok(projected.selected_absence_proved)
}

impl Summary {
    fn add(
        &mut self,
        archive: &Archive,
        config: Option<Guid>,
        written: Option<String>,
        target: PlacedTarget,
    ) {
        if let Some(found) = target.entry() {
            self.nested.push(found);
        }
        if let Some(config) = config {
            self.placed
                .push(configured_placement(archive, config, written, target));
        }
    }
}

fn configured_placement(
    archive: &Archive,
    config: Guid,
    written: Option<String>,
    target: PlacedTarget,
) -> Placed {
    let path = target.entry().map_or_else(
        || written.unwrap_or_default(),
        |found| path_of(archive, found).to_owned(),
    );
    Placed {
        config,
        path,
        target,
    }
}

fn resolve_target(
    archive: &Archive,
    index: &Index<'_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    selector: AuthoredEntryPath,
    absence_proved: bool,
) -> Result<PlacedTarget, CatalogError> {
    match index.candidates(selector.as_str()) {
        [entry] => Ok(PlacedTarget::Resolved(*entry)),
        [] if absence_proved => {
            EntrySource::authored_missing(selector).record(
                ledger,
                Kind::LootContainers,
                SourceDisposition::VerifiedMissingReference,
            );
            Ok(PlacedTarget::VerifiedMissing)
        }
        [] => {
            EntrySource::authored_missing(selector).record(
                ledger,
                Kind::LootContainers,
                SourceDisposition::ReadFailed,
            );
            Ok(PlacedTarget::Missing)
        }
        candidates => {
            entry_source::record_ambiguous_outer_entries(
                archive,
                capture,
                ledger,
                Kind::LootContainers,
                Some(&selector),
                candidates,
            )?;
            Ok(PlacedTarget::Ambiguous)
        }
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
