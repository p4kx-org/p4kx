// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Archive, CancelToken};
use rayon::prelude::*;

use crate::catalog_read::original::EntryCapture;
use crate::contract_loot::{LootContainer, TagCount};
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::harvest_locations::Objects;
use crate::loot_soc::EntityTags;
use crate::loot_unreadable::{Unreadable, UnreadableContainer};
use crate::loot_walk::{Explored, Placed, Summary, below, explore, path_of};
use crate::starmap_index::Index;
use crate::starmap_walk::Opened;
use crate::{CatalogError, Kind};

/// The tag counts of a placed container, or why they cannot be counted.
type Tags = Result<EntityTags, Unreadable>;

/// The loot containers found and what could not be read or used while looking for them.
pub(crate) struct Located {
    pub(crate) ledger: ExportLedger,
    pub(crate) containers: Vec<LootContainer>,
    /// The containers and entity files that cannot be read, ordered by path.
    pub(crate) unreadable: Vec<UnreadableContainer>,
    /// How many containers the limit on the walk left unread.
    pub(crate) beyond_limit: usize,
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
        beyond_limit,
        mut ledger,
    } = explore(archive, &index, opened, cancel, capture)?;
    let (tags, observed) = read_tags(archive, &summaries, cancel, capture)?;
    ledger.merge(observed);
    let containers = rows(opened, objects, &summaries, &tags);
    if !cancel.is_some_and(CancelToken::is_cancelled) {
        ledger.inventory(Kind::LootContainers);
    }
    Ok(Located {
        ledger,
        containers,
        unreadable: unreadable(archive, &summaries, &tags),
        beyond_limit,
        several: several(archive, &summaries),
    })
}

fn rows(
    opened: &[Opened],
    objects: &Objects,
    summaries: &HashMap<usize, Summary>,
    tags: &HashMap<usize, Tags>,
) -> Vec<LootContainer> {
    let mut reached: HashMap<usize, Vec<&Placed>> = HashMap::new();
    let mut containers = Vec::new();
    for root in opened {
        let location = objects.location(root.under).ok();
        let placed = reached
            .entry(root.entry)
            .or_insert_with(|| below(root.entry, summaries));
        for placed in placed.iter() {
            containers.push(row(
                placed,
                &root.system,
                location,
                placed.entry.and_then(|entry| tags.get(&entry)),
            ));
        }
    }
    containers.sort_by_cached_key(|row| {
        (
            row.multi_config_guid.text(),
            row.container.clone(),
            row.system.clone(),
            row.starmap_object_guid.map(CatalogGuid::text),
        )
    });
    containers.dedup_by(|later, earlier| {
        later.multi_config_guid == earlier.multi_config_guid
            && later.container == earlier.container
            && later.system == earlier.system
            && later.starmap_object_guid == earlier.starmap_object_guid
    });
    containers
}

/// Returns the containers that cannot be read, a placed container the Archive lacks, and the
/// tags that cannot be counted, ordered by path and reason.
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
        .filter(|placed| placed.entry.is_none())
        .map(|placed| UnreadableContainer {
            path: placed.path.clone(),
            reason: Unreadable::MissingFromArchive,
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
) -> LootContainer {
    let counts = tags.and_then(|read| read.as_ref().ok());
    let mut tag_counts: Vec<TagCount> = counts
        .map(|counts| {
            counts
                .by_tag
                .iter()
                .map(|(&tag, &entities)| TagCount {
                    tag_guid: CatalogGuid(tag),
                    entities,
                })
                .collect()
        })
        .unwrap_or_default();
    tag_counts.sort_by_cached_key(|count| count.tag_guid.text());
    LootContainer {
        multi_config_guid: CatalogGuid(placed.config),
        container: placed.path.clone(),
        system: system.to_owned(),
        starmap_object_guid: location,
        entity_count: counts.map(|counts| counts.entities),
        tag_counts,
    }
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
        .flat_map(|summary| summary.placed.iter().filter_map(|placed| placed.entry))
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
mod tests {
    use super::*;

    use p4kx_extraction::Guid;

    const CONFIG: &str = "687af3db-4c0d-4e8d-9261-102274f8b6ed";

    fn placed(path: &str) -> Placed {
        Placed {
            config: CONFIG.parse::<Guid>().expect("a GUID"),
            path: path.to_owned(),
            entry: Some(1),
        }
    }

    #[test]
    fn a_container_that_could_not_be_counted_has_no_entity_count() {
        let failed: Tags = Err(Unreadable::SocFile);
        let row = row(&placed("a.socpak"), "stanton", None, Some(&failed));
        assert_eq!(row.entity_count, None);
        assert!(row.tag_counts.is_empty());
    }

    #[test]
    fn a_container_without_a_soc_has_zero_entities() {
        let none: Tags = Ok(EntityTags::default());
        let row = row(&placed("a.socpak"), "stanton", None, Some(&none));
        assert_eq!(row.entity_count, Some(0));
    }

    #[test]
    fn a_container_the_archive_lacks_has_no_entity_count() {
        let row = row(&placed("a.socpak"), "stanton", None, None);
        assert_eq!(row.entity_count, None);
    }
}
