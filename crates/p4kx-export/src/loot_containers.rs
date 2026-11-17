// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_extraction::{Archive, CancelToken};
use rayon::prelude::*;

use crate::catalog_read::original::EntryCapture;
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
) -> Vec<LootContainer> {
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
            )
        })
        .collect()
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
        .filter_map(|placed| {
            let reason = match placed.target {
                PlacedTarget::Resolved(_) => return None,
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
) -> LootContainer {
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
mod tests {
    use std::collections::HashMap;

    use p4kx_extraction::Guid;

    use super::{Tags, row, rows};
    use crate::guid_text::CatalogGuid;
    use crate::harvest_locations::Objects;
    use crate::loot_soc::EntityTags;
    use crate::loot_unreadable::Unreadable;
    use crate::loot_walk::{Placed, PlacedTarget, Summary};
    use crate::starmap_walk::Opened;

    const CONFIG: &str = "687af3db-4c0d-4e8d-9261-102274f8b6ed";

    fn placed(path: &str) -> Placed {
        Placed {
            config: CONFIG.parse::<Guid>().expect("a GUID"),
            path: path.to_owned(),
            target: PlacedTarget::Resolved(1),
        }
    }

    fn placement_summaries(first_config: Guid) -> HashMap<usize, Summary> {
        let alternate = Placed {
            target: PlacedTarget::Resolved(2),
            ..placed("a.socpak")
        };
        HashMap::from([
            (
                10,
                Summary {
                    placed: vec![placed("b.socpak"), placed("a.socpak")],
                    ..Summary::default()
                },
            ),
            (
                20,
                Summary {
                    placed: vec![
                        alternate,
                        Placed {
                            config: first_config,
                            ..placed("z.socpak")
                        },
                    ],
                    ..Summary::default()
                },
            ),
        ])
    }

    fn opened(entry: usize, system: &str) -> Opened {
        Opened {
            system: system.into(),
            entry,
            under: None,
        }
    }

    #[test]
    fn duplicate_rows_keep_the_first_placement_with_canonical_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let first_config: Guid = "67000000-0000-00ff-0000-000000000000".parse()?;
        let second_config: Guid = CONFIG.parse()?;
        let summaries = placement_summaries(first_config);
        let roots = [
            opened(10, "stanton"),
            opened(20, "stanton"),
            opened(20, "nyx"),
        ];
        let tags = HashMap::from([
            (
                1,
                Ok(EntityTags {
                    entities: 3,
                    by_tag: HashMap::from([(first_config, 2)]),
                }),
            ),
            (
                2,
                Ok(EntityTags {
                    entities: 5,
                    by_tag: HashMap::from([(second_config, 4)]),
                }),
            ),
        ]);
        let result = rows(&roots, &Objects::new(&[]), &summaries, &tags);
        let signatures: Vec<_> = result
            .iter()
            .map(|row| {
                (
                    row.multi_config_guid.0,
                    row.container.as_str(),
                    row.system.as_str(),
                    row.entity_count,
                )
            })
            .collect();
        assert_eq!(
            signatures,
            [
                (first_config, "z.socpak", "nyx", Some(3)),
                (first_config, "z.socpak", "stanton", Some(3)),
                (second_config, "a.socpak", "nyx", Some(5)),
                (second_config, "a.socpak", "stanton", Some(3)),
                (second_config, "b.socpak", "stanton", Some(3)),
            ]
        );
        let stanton = result
            .iter()
            .find(|row| row.system == "stanton" && row.container == "a.socpak")
            .ok_or("deduplicated Stanton placement")?;
        let [count] = stanton.tag_counts.as_slice() else {
            return Err("expected one tag from the first placement".into());
        };
        assert_eq!(
            (count.tag_guid, count.entities),
            (CatalogGuid(first_config), 2)
        );
        Ok(())
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
