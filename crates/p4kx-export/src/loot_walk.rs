// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The walk through the containers that entities of containers place.

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{Archive, CancelToken, Guid};
use rayon::prelude::*;

use crate::catalog_read::original::EntryCapture;
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::loot_entxml::EntityLink;
use crate::loot_unreadable::Unreadable;
use crate::starmap_index::Index;
use crate::starmap_walk::Opened;
use crate::{CatalogError, ExportAccountingError, Kind};

/// The most distinct containers followed from the system containers.
pub(crate) const MAX_CONTAINERS: usize = 1 << 18;

/// An entity of a container that carries a multi config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Placed {
    pub(crate) config: Guid,
    /// The path of the placed container: the Archive's when it has it, else as written.
    pub(crate) path: String,
    /// The Archive entry of the placed container, `None` when the Archive lacks it.
    pub(crate) entry: Option<usize>,
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

/// The containers read, by Archive entry, and how many distinct containers the limit left unread.
pub(crate) struct Explored {
    pub(crate) ledger: ExportLedger,
    pub(crate) summaries: HashMap<usize, Summary>,
    pub(crate) beyond_limit: usize,
}

/// Reads the containers in `opened` and every container their entities place, breadth first,
/// each once, until none is left or [`MAX_CONTAINERS`] are read.
pub(crate) fn explore(
    archive: &Archive,
    index: &Index<'_>,
    opened: &[Opened],
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<Explored, CatalogError> {
    explore_within(archive, index, opened, cancel, MAX_CONTAINERS, capture)
}

fn explore_within(
    archive: &Archive,
    index: &Index<'_>,
    opened: &[Opened],
    cancel: Option<&CancelToken>,
    limit: usize,
    capture: &EntryCapture<'_>,
) -> Result<Explored, CatalogError> {
    let mut ledger = ExportLedger::default();
    let mut summaries: HashMap<usize, Summary> = HashMap::new();
    let mut unread: HashSet<usize> = HashSet::new();
    let mut frontier: Vec<usize> = opened.iter().map(|root| root.entry).collect();
    while !frontier.is_empty() && !cancel.is_some_and(CancelToken::is_cancelled) {
        frontier.sort_unstable();
        frontier.dedup();
        frontier.retain(|entry| !summaries.contains_key(entry));
        let room = limit.saturating_sub(summaries.len());
        if frontier.len() > room {
            unread.extend(frontier.split_off(room));
        }
        let read: Vec<_> = frontier
            .par_iter()
            .filter(|_| !cancel.is_some_and(CancelToken::is_cancelled))
            .map(|&entry| summarise(archive, index, entry, capture).map(|read| (entry, read)))
            .collect::<Result<Vec<_>, _>>()?;
        frontier = read
            .iter()
            .flat_map(|(_, (summary, _))| summary.nested.iter().copied())
            .collect();
        for (entry, (summary, observed)) in read {
            summaries.insert(entry, summary);
            ledger.merge(observed);
        }
    }
    for entry in &unread {
        if let Some(entry) = archive.entries().get(*entry) {
            EntrySource::outer(capture, archive, entry)?
                .operation(EntryOperation::Enumeration)
                .record(
                    &mut ledger,
                    Kind::LootContainers,
                    SourceDisposition::ReadFailed,
                );
        }
    }
    if !unread.is_empty() {
        ledger.incomplete_inventory(Kind::LootContainers);
    }
    Ok(Explored {
        ledger,
        summaries,
        beyond_limit: unread.len(),
    })
}

fn summarise(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    capture: &EntryCapture<'_>,
) -> Result<(Summary, ExportLedger), CatalogError> {
    let read = crate::loot_container_source::read(archive, entry, capture)?;
    let mut ledger = read.ledger;
    let mut summary = Summary {
        unreadable: read.unreadable,
        ..Summary::default()
    };
    for (path, link) in read.links {
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
        let found = written.as_deref().and_then(|path| index.find(path));
        if let Some(path) = written.as_deref().filter(|_| found.is_none()) {
            let selected = selected.ok_or(ExportAccountingError::UnboundHealthSubject)?;
            if selected.as_str() != path {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            EntrySource::authored_missing(selected).record(
                &mut ledger,
                Kind::LootContainers,
                SourceDisposition::ReadFailed,
            );
        }
        if let Some(found) = found {
            summary.nested.push(found);
        }
        if let Some(config) = config {
            let path = match (found, written) {
                (Some(found), _) => path_of(archive, found).to_owned(),
                (None, written) => written.unwrap_or_default(),
            };
            summary.placed.push(Placed {
                config,
                path,
                entry: found,
            });
        }
    }
    summary.nested.sort_unstable();
    summary.nested.dedup();
    Ok((summary, ledger))
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
