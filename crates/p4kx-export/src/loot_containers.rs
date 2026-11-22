// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};
use std::slice::from_ref;

use p4kx_extraction::{Archive, CancelToken, SourceUses, SourceValue, SourcedValue};
use rayon::prelude::*;

use crate::catalog_read::original::EntryCapture;
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::contract_loot::{LootContainer, TagCount};
use crate::export_ledger::ExportLedger;
use crate::guid_text::{CatalogGuid, GuidText};
use crate::harvest_locations::{NoObject, Objects};
use crate::loot_soc_source::Counted as Tags;
use crate::loot_unreadable::{Unreadable, UnreadableContainer};
use crate::loot_walk::{Explored, Placed, PlacedTarget, Summary, below, explore, path_of};
use crate::starmap_index::Index;
use crate::starmap_walk::Opened;
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "loot_parent_origin.rs"]
mod parent_origin;
use parent_origin::ParentOrigins;

#[path = "loot_system_origin.rs"]
mod system_origin;

use system_origin::SystemOrigins;

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
    objects: &Objects<'_>,
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
    let mut systems = SystemOrigins::try_new(archive, capture)?;
    let mut parents = ParentOrigins::new(objects, capture.source_uses());
    let containers = rows(
        opened,
        objects,
        &summaries,
        &tags,
        capture.source_uses(),
        |root, location| {
            let system = systems.project(root)?;
            let object = parents.project(root, location, &system)?;
            Ok(PlacementValues { system, object })
        },
    )?;
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

struct PlacementValues {
    system: SourcedValue<String>,
    object: SourcedValue<CatalogGuid>,
}

fn rows(
    opened: &[Opened],
    objects: &Objects<'_>,
    summaries: &HashMap<usize, Summary>,
    tags: &HashMap<usize, Tags>,
    sources: &SourceUses,
    mut placement: impl FnMut(
        &Opened,
        Result<CatalogGuid, NoObject>,
    ) -> Result<PlacementValues, CatalogError>,
) -> Result<Vec<LootContainer>, CatalogError> {
    let mut reached: HashMap<usize, Vec<&Placed>> = HashMap::new();
    let mut selected = BTreeMap::new();
    for root in opened {
        let location = objects.location(root.under);
        let object = location.ok().map(CatalogGuid::text);
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
            selected.entry(key).or_insert((root, placed, location));
        }
    }
    drop(reached);
    selected
        .into_values()
        .map(|(root, placed, location)| {
            row(
                placed,
                placement(root, location)?,
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
        read.tags.as_ref().err().map(|&reason| UnreadableContainer {
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
    placement: PlacementValues,
    tags: Option<&Tags>,
    sources: &SourceUses,
) -> Result<LootContainer, CatalogError> {
    let entity_count = count_value(placed, tags, sources)?;
    let tag_counts = project_tag_counts(&entity_count, tags, sources)?;
    Ok(LootContainer {
        multi_config_guid: sources.value_with_origins(
            SourceValue::Present(CatalogGuid(placed.config)),
            &[&placed.config_origin],
        )?,
        container: sources.value_with_origins(
            SourceValue::Present(placed.path.clone()),
            &[&placed.path_origin],
        )?,
        system: placement.system,
        starmap_object_guid: placement.object,
        entity_count,
        tag_counts,
    })
}

fn project_tag_counts(
    survey: &SourcedValue<u32>,
    tags: Option<&Tags>,
    sources: &SourceUses,
) -> Result<SourcedValue<Vec<TagCount>>, CatalogError> {
    let mut rows = Vec::new();
    if let Some(read) = tags
        && let Ok(counts) = &read.tags
    {
        rows.try_reserve(counts.by_tag.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for (&guid, &count) in &counts.by_tag {
            let use_ref = read
                .first_tag_uses
                .get(&guid)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            rows.push(TagCount {
                tag_guid: sources.value(
                    SourceValue::Present(CatalogGuid(guid)),
                    from_ref(use_ref),
                    true,
                ),
                entities: survey.map_ref(|_| count),
            });
        }
        rows.sort_by_cached_key(|row| row.tag_guid.value().copied().map(CatalogGuid::text));
    }
    Ok(survey.map_ref(|_| rows))
}

fn count_value(
    placed: &Placed,
    tags: Option<&Tags>,
    sources: &SourceUses,
) -> Result<SourcedValue<u32>, CatalogError> {
    if let Some(read) = tags {
        let value = read.tags.as_ref().map_or(SourceValue::Unreadable, |tags| {
            SourceValue::Present(tags.entities)
        });
        return Ok(sources.value_with_origins(value, &[&placed.path_origin, &read.origin])?);
    }
    if matches!(placed.target, PlacedTarget::VerifiedMissing) {
        return Ok(sources.value_with_origins(SourceValue::Absent, &[&placed.path_origin])?);
    }
    let incomplete = sources.origin(&[], false);
    Ok(sources.value_with_origins(SourceValue::Unreadable, &[&placed.path_origin, &incomplete])?)
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
