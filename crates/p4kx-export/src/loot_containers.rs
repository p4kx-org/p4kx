// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_extraction::{Archive, CancelToken, SourceUses, SourceValue};
use rayon::prelude::*;

use crate::catalog_read::original::EntryCapture;
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::contract_loot::{LootContainer, TagCount};
use crate::export_ledger::ExportLedger;
use crate::guid_text::{CatalogGuid, GuidText};
use crate::harvest_locations::Objects;
use crate::loot_soc::EntityTags;
use crate::loot_unreadable::{Unreadable, UnreadableContainer};
use crate::loot_walk::{Explored, Placed, PlacedTarget, Summary, below, explore, path_of};
use crate::starmap_index::Index;
use crate::starmap_walk::Opened;
use crate::{CatalogError, Kind};

/// The tag counts of a placed container, or why they cannot be counted.
type Tags = Result<EntityTags, Unreadable>;

/// The loot containers found and what could not be read or used while looking for them.
pub(crate) struct Located {
    pub(crate) ledger: ExportLedger,
    pub(crate) containers: Vec<LootContainer>,
    pub(crate) original_documents: StagedCatalogTable,
    /// The containers and entity files that cannot be read, ordered by path.
    pub(crate) unreadable: Vec<UnreadableContainer>,
    /// The entity files that name several different containers or configs, as `container: file`,
    /// ordered.
    pub(crate) several: Vec<String>,
}

/// Returns the containers that carry a multi config in or below the containers `opened`, each
/// with the starmap object the root container hangs below. A container is followed through the
/// containers its entities place (`objectContainer`) and read once, in parallel; one placed below
/// several roots gives a row for each root. The rows are ordered by config, container, system and
/// object.
pub(crate) fn locate(
    archive: &Archive,
    opened: &[Opened],
    objects: &Objects,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<Located, CatalogError> {
    let index = Index::new(archive);
    let Explored {
        summaries,
        mut ledger,
        original_documents,
    } = explore(archive, &index, opened, cancel, capture)?;
    let (tags, observed) = read_tags(archive, &summaries, cancel, capture)?;
    ledger.merge(observed);
    let containers = rows(opened, objects, &summaries, &tags, capture.source_uses())?;
    if !cancel.is_some_and(CancelToken::is_cancelled) {
        ledger.inventory(Kind::LootContainers);
    }
    Ok(Located {
        ledger,
        containers,
        original_documents,
        unreadable: unreadable(archive, &summaries, &tags),
        several: several(archive, &summaries),
    })
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ContainerKey<'source> {
    config: GuidText,
    container: &'source str,
    system: &'source str,
    object: Option<GuidText>,
}

fn rows(
    opened: &[Opened],
    objects: &Objects,
    summaries: &HashMap<usize, Summary>,
    tags: &HashMap<usize, Tags>,
    sources: &SourceUses,
) -> Result<Vec<LootContainer>, CatalogError> {
    let mut reached: HashMap<usize, Vec<&Placed>> = HashMap::new();
    let mut selected = BTreeMap::new();
    for root in opened {
        let location = objects.location(root.under).ok();
        let object = location.map(CatalogGuid::text);
        let reached_placements = reached
            .entry(root.entry)
            .or_insert_with(|| below(root.entry, summaries));
        for &placed in reached_placements.as_slice() {
            let key = ContainerKey {
                config: CatalogGuid(placed.config).text(),
                container: &placed.path,
                system: &root.system,
                object,
            };
            selected.entry(key).or_insert((placed, location));
        }
    }
    drop(reached);
    selected
        .into_iter()
        .map(|(key, (placed, location))| {
            row(
                placed,
                key.system,
                location,
                placed.target.entry().and_then(|entry| tags.get(&entry)),
                sources,
            )
        })
        .collect()
}

/// Returns the unreadable sources and unresolved targets, ordered by path and reason.
fn unreadable(
    archive: &Archive,
    summaries: &HashMap<usize, Summary>,
    tags: &HashMap<usize, Tags>,
) -> Vec<UnreadableContainer> {
    let read = summaries.iter().filter_map(|(&entry, summary)| {
        summary.unreadable.map(|reason| UnreadableContainer {
            path: path_of(archive, entry).to_owned(),
            reason,
        })
    });
    let counted = tags.iter().filter_map(|(&entry, read)| {
        read.as_ref().err().map(|&reason| UnreadableContainer {
            path: path_of(archive, entry).to_owned(),
            reason,
        })
    });
    let missing = summaries
        .values()
        .flat_map(|summary| summary.placed.iter())
        .filter_map(|placed| {
            let reason = match placed.target {
                PlacedTarget::Resolved(_) | PlacedTarget::VerifiedMissing => return None,
                PlacedTarget::Missing => Unreadable::MissingFromArchive,
                PlacedTarget::Ambiguous => Unreadable::AmbiguousContainer,
            };
            Some(UnreadableContainer {
                path: placed.path.clone(),
                reason,
            })
        });
    let mut found: Vec<UnreadableContainer> = read.chain(counted).chain(missing).collect();
    found.sort_by_cached_key(ToString::to_string);
    found.dedup();
    found
}

fn several(archive: &Archive, summaries: &HashMap<usize, Summary>) -> Vec<String> {
    let mut files: Vec<String> = summaries
        .iter()
        .flat_map(|(&entry, summary)| {
            summary
                .several
                .iter()
                .map(move |file| format!("{}: {file}", path_of(archive, entry)))
        })
        .collect();
    files.sort_unstable();
    files
}

fn row(
    placed: &Placed,
    system: &str,
    location: Option<CatalogGuid>,
    tags: Option<&Tags>,
    sources: &SourceUses,
) -> Result<LootContainer, CatalogError> {
    let counts = tags.and_then(|read| read.as_ref().ok());
    let mut tag_counts: Vec<TagCount> = counts.map_or_default(|counts| {
        counts
            .by_tag
            .iter()
            .map(|(&tag, &entities)| TagCount {
                tag_guid: CatalogGuid(tag),
                entities,
            })
            .collect()
    });
    tag_counts.sort_by_cached_key(|count| count.tag_guid.text());
    Ok(LootContainer {
        multi_config_guid: sources.value_with_origins(
            SourceValue::Present(CatalogGuid(placed.config)),
            &[&placed.config_origin],
        )?,
        container: sources.value_with_origins(
            SourceValue::Present(placed.path.clone()),
            &[&placed.path_origin],
        )?,
        system: system.to_owned(),
        starmap_object_guid: location,
        entity_count: counts.map(|counts| counts.entities),
        tag_counts,
    })
}

/// Counts the entities and tags of all `.soc` files of every container a config is placed with,
/// in parallel.
fn read_tags(
    archive: &Archive,
    summaries: &HashMap<usize, Summary>,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<(HashMap<usize, Tags>, ExportLedger), CatalogError> {
    let mut entries: Vec<usize> = summaries
        .values()
        .flat_map(|summary| {
            summary
                .placed
                .iter()
                .filter_map(|placed| placed.target.entry())
        })
        .collect();
    entries.sort_unstable();
    entries.dedup();
    let reads: Vec<_> = entries
        .par_iter()
        .filter(|_| !cancel.is_some_and(CancelToken::is_cancelled))
        .map(|&entry| {
            crate::loot_soc_source::read(archive, entry, cancel, capture).map(|read| (entry, read))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut ledger = ExportLedger::default();
    let mut tags = HashMap::with_capacity(reads.len());
    for (entry, (counted, observed)) in reads {
        tags.insert(entry, counted);
        ledger.merge(observed);
    }
    Ok((tags, ledger))
}

#[cfg(test)]
#[path = "loot_container_row_tests.rs"]
mod tests;
