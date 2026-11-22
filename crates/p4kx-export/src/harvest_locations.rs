// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};

use p4kx_extraction::{Archive, CancelToken, Guid, SourcedValue};
use rayon::prelude::*;

use crate::catalog_read::original::EntryCapture;
use crate::contract::StarmapObject;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::mining_locations::HarvestableLocation;
use crate::starmap_walk::Opened;
use crate::{CatalogError, Kind};

/// What the first `.soc` of a container says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Soc {
    Preset(Guid),
    Plain,
    Unreadable(&'static str),
}

/// Why a container has no starmap object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum NoObject {
    /// No object is recorded above the container: no Child reaches it, or it is reached with no
    /// object above, as a system root.
    NoneAbove,
    /// The object the container hangs below is not in the starmap objects.
    NotInCatalog,
    /// Mining claim markers form a cycle above the container.
    MarkerCycle,
    /// A mining claim marker is the top of the chain, with no object above it.
    MarkerWithoutParent,
}

impl NoObject {
    /// Returns the reason as the warning words it.
    pub(crate) const fn reason(self) -> &'static str {
        match self {
            Self::NoneAbove => "no object is recorded above the container",
            Self::NotInCatalog => {
                "the object the container hangs below is not in the starmap objects"
            }
            Self::MarkerCycle => "mining claim markers form a cycle above the container",
            Self::MarkerWithoutParent => "a mining claim marker has no object above it",
        }
    }

    /// Whether the reason is a loss the caller should hear of: a container with no object above
    /// it is the normal case and stays silent.
    pub(crate) const fn is_loss(self) -> bool {
        !matches!(self, Self::NoneAbove)
    }
}

/// The container locations found, the containers that could not be read, each as its path and
/// the reason, ordered by path, and how many locations lack an object for each reason that is a
/// loss, ordered by reason.
pub(crate) struct Located {
    pub(crate) ledger: ExportLedger,
    pub(crate) locations: Vec<HarvestableLocation>,
    pub(crate) unreadable: Vec<String>,
    pub(crate) without_object: Vec<(NoObject, usize)>,
}

/// The starmap objects a container can hang below, by GUID: whether the object is a mining claim
/// marker and the object above it.
pub(crate) struct Objects {
    by_guid: HashMap<Guid, (bool, Option<Guid>)>,
}

impl Objects {
    pub(crate) fn new<'object, 'source: 'object>(
        objects: impl IntoIterator<Item = &'object StarmapObject<'source>>,
    ) -> Self {
        Self {
            by_guid: objects
                .into_iter()
                .map(|object| {
                    (
                        object.guid.0,
                        (
                            object.mining_claim,
                            object.placed_under_guid.map(|guid| guid.0),
                        ),
                    )
                })
                .collect(),
        }
    }

    /// Returns the object a container hangs below: `under`, else the object above it while it is
    /// a mining claim marker; the reason when there is none.
    pub(crate) fn location(&self, under: Option<Guid>) -> Result<CatalogGuid, NoObject> {
        let mut at = under.ok_or(NoObject::NoneAbove)?;
        let mut fast = Some(at);
        loop {
            let &(mining_claim, above) = self.by_guid.get(&at).ok_or(NoObject::NotInCatalog)?;
            if !mining_claim {
                return Ok(CatalogGuid(at));
            }
            at = above.ok_or(NoObject::MarkerWithoutParent)?;
            // The fast cursor detects cycles; the main cursor classifies terminal failures.
            fast = fast
                .and_then(|node| self.marker_parent(node))
                .and_then(|node| self.marker_parent(node));
            if fast == Some(at) {
                return Err(NoObject::MarkerCycle);
            }
        }
    }

    fn marker_parent(&self, at: Guid) -> Option<Guid> {
        match self.by_guid.get(&at) {
            Some(&(true, above)) => above,
            _ => None,
        }
    }
}

/// Returns the provider preset every container in `opened` names, for the presets in `presets`
/// only. Each container is read once, in parallel; a container hung below several objects gives
/// one location for each. The locations are ordered by preset, container, system and object.
pub(crate) fn locate(
    archive: &Archive,
    opened: &[Opened],
    presets: &HashSet<Guid>,
    objects: &Objects,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<Located, CatalogError> {
    let mut entries: Vec<usize> = opened.iter().map(|container| container.entry).collect();
    entries.sort_unstable();
    entries.dedup();
    let (read, ledger) = read_sources(archive, &entries, cancel, capture)?;
    Ok(locate_from_sources(
        archive, opened, presets, objects, cancel, &read, ledger,
    ))
}

fn locate_from_sources(
    archive: &Archive,
    opened: &[Opened],
    presets: &HashSet<Guid>,
    objects: &Objects,
    cancel: Option<&CancelToken>,
    read: &HashMap<usize, Soc>,
    mut ledger: ExportLedger,
) -> Located {
    let mut unreadable: Vec<String> = read
        .iter()
        .filter_map(|(&entry, soc)| match soc {
            Soc::Unreadable(reason) => Some(format!("{}: {reason}", path_of(archive, entry))),
            _ => None,
        })
        .collect();
    unreadable.sort_unstable();
    let mut rows: Vec<Row> = opened
        .iter()
        .filter_map(|container| {
            let Some(Soc::Preset(preset)) = read.get(&container.entry) else {
                return None;
            };
            let object = objects.location(container.under);
            presets.contains(preset).then(|| {
                (
                    HarvestableLocation {
                        preset_guid: *preset,
                        starmap_object_guid: object.ok().map(CatalogGuid::get),
                        system: container.system.to_string(),
                        container: path_of(archive, container.entry).to_owned(),
                    },
                    object.err(),
                )
            })
        })
        .collect();
    let authored_rows = rows.len();
    sort_unique(&mut rows);
    ledger.skipped_rows(Kind::HarvestableLocations, authored_rows - rows.len());
    if !cancel.is_some_and(CancelToken::is_cancelled) {
        ledger.inventory(Kind::HarvestableLocations);
    }
    Located {
        ledger,
        without_object: losses(&rows),
        locations: rows.into_iter().map(|(location, _)| location).collect(),
        unreadable,
    }
}

fn read_sources(
    archive: &Archive,
    entries: &[usize],
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<(HashMap<usize, Soc>, ExportLedger), CatalogError> {
    let reads: Vec<_> = entries
        .par_iter()
        .filter(|_| !cancel.is_some_and(CancelToken::is_cancelled))
        .map(|&entry| {
            crate::harvest_source::read(archive, entry, capture, cancel).map(|read| (entry, read))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut ledger = ExportLedger::default();
    let mut sources = HashMap::with_capacity(reads.len());
    for (entry, (soc, observed)) in reads {
        ledger.merge(observed);
        sources.insert(entry, soc);
    }
    Ok((sources, ledger))
}

/// Selected provider observations memoized by physical outer Entry index.
pub(crate) struct HarvestSurvey<'providers> {
    presets: &'providers HashSet<Guid>,
    reads: RefCell<SurveyReads>,
}

#[derive(Default)]
struct SurveyReads {
    by_entry: HashMap<usize, Soc>,
    selected: HashMap<usize, SourcedValue<Guid>>,
    ledger: ExportLedger,
}

impl<'providers> HarvestSurvey<'providers> {
    pub(crate) fn new(presets: &'providers HashSet<Guid>) -> Self {
        Self {
            presets,
            reads: RefCell::default(),
        }
    }

    pub(crate) fn probe(
        &self,
        archive: &Archive,
        entry: usize,
        capture: &EntryCapture<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Guid>, CatalogError> {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        if self.presets.is_empty() {
            return Ok(None);
        }
        let mut reads = self.reads.borrow_mut();
        let SurveyReads {
            by_entry,
            selected,
            ledger,
        } = &mut *reads;
        if let Entry::Vacant(slot) = by_entry.entry(entry) {
            let (observed, observations) = crate::harvest_source::read_verified(
                archive,
                entry,
                capture,
                cancel,
                self.presets,
            )?;
            slot.insert(observed.fact);
            if let Some(preset) = observed.selected {
                selected.insert(entry, preset);
            }
            ledger.merge(observations);
        }
        Ok(selected.get(&entry).and_then(SourcedValue::value).copied())
    }

    pub(crate) fn finish(
        self,
        archive: &Archive,
        opened: &[Opened],
        objects: &Objects,
        cancel: Option<&CancelToken>,
        capture: &EntryCapture<'_>,
    ) -> Result<(Located, HashMap<usize, SourcedValue<Guid>>), CatalogError> {
        for occurrence in opened {
            let _ = self.probe(archive, occurrence.entry, capture, cancel)?;
        }
        let SurveyReads {
            by_entry,
            selected,
            ledger,
        } = self.reads.into_inner();
        let located = locate_from_sources(
            archive,
            opened,
            self.presets,
            objects,
            cancel,
            &by_entry,
            ledger,
        );
        Ok((located, selected))
    }
}

/// A location and why it has no object, `None` when it has one.
type Row = (HarvestableLocation, Option<NoObject>);

/// Orders `rows` by preset, container, system and object, and drops the repeats.
fn sort_unique(rows: &mut Vec<Row>) {
    rows.sort_by(|(a, _), (b, _)| {
        CatalogGuid(a.preset_guid)
            .text()
            .cmp(&CatalogGuid(b.preset_guid).text())
            .then_with(|| a.container.cmp(&b.container))
            .then_with(|| a.system.cmp(&b.system))
            .then_with(|| {
                let text = |location: &HarvestableLocation| {
                    location
                        .starmap_object_guid
                        .map(CatalogGuid)
                        .map(CatalogGuid::text)
                };
                text(a).cmp(&text(b))
            })
    });
    rows.dedup_by(|(later, _), (earlier, _)| {
        later.preset_guid == earlier.preset_guid
            && later.container == earlier.container
            && later.system == earlier.system
            && later.starmap_object_guid == earlier.starmap_object_guid
    });
}

/// Counts the rows without an object for each reason that is a loss, ordered by reason.
fn losses(rows: &[Row]) -> Vec<(NoObject, usize)> {
    let mut counts: BTreeMap<NoObject, usize> = BTreeMap::new();
    for reason in rows
        .iter()
        .filter_map(|(_, reason)| *reason)
        .filter(|reason| reason.is_loss())
    {
        *counts.entry(reason).or_default() += 1;
    }
    counts.into_iter().collect()
}

fn path_of(archive: &Archive, entry: usize) -> &str {
    archive
        .entries()
        .get(entry)
        .map_or("", p4kx_extraction::Entry::path)
}

#[cfg(test)]
#[path = "harvest_location_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "harvest_location_chain_tests.rs"]
mod chain_tests;
