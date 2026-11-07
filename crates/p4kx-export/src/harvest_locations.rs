// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap, HashSet};

use p4kx_extraction::{Archive, CancelToken, Guid};
use rayon::prelude::*;

use crate::Kind;
use crate::contract::StarmapObject;
use crate::contract_harvest::HarvestableLocation;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::starmap_walk::Opened;

/// The most objects followed upwards from a container to find the object it belongs to.
const MAX_CHAIN: usize = 64;

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
    /// [`MAX_CHAIN`] or more mining claim markers lie between the container and the first object
    /// that is none.
    ChainTooLong,
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
            Self::ChainTooLong => "64 or more mining claim markers lie above the container",
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
    pub(crate) fn new(objects: &[StarmapObject<'_>]) -> Self {
        Self {
            by_guid: objects
                .iter()
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
        for _ in 0..MAX_CHAIN {
            let &(mining_claim, above) = self.by_guid.get(&at).ok_or(NoObject::NotInCatalog)?;
            if !mining_claim {
                return Ok(CatalogGuid(at));
            }
            at = above.ok_or(NoObject::MarkerWithoutParent)?;
        }
        Err(NoObject::ChainTooLong)
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
) -> Located {
    let mut entries: Vec<usize> = opened.iter().map(|container| container.entry).collect();
    entries.sort_unstable();
    entries.dedup();
    let (read, mut ledger) = read_sources(archive, &entries, cancel);
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
                        preset_guid: CatalogGuid(*preset),
                        starmap_object_guid: object.ok(),
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
) -> (HashMap<usize, Soc>, ExportLedger) {
    let reads: Vec<_> = entries
        .par_iter()
        .filter(|_| !cancel.is_some_and(CancelToken::is_cancelled))
        .map(|&entry| (entry, crate::harvest_source::read(archive, entry)))
        .collect();
    let mut ledger = ExportLedger::default();
    let mut sources = HashMap::with_capacity(reads.len());
    for (entry, (soc, observed)) in reads {
        ledger.merge(observed);
        sources.insert(entry, soc);
    }
    (sources, ledger)
}

/// A location and why it has no object, `None` when it has one.
type Row = (HarvestableLocation, Option<NoObject>);

/// Orders `rows` by preset, container, system and object, and drops the repeats.
fn sort_unique(rows: &mut Vec<Row>) {
    rows.sort_by(|(a, _), (b, _)| {
        a.preset_guid
            .text()
            .cmp(&b.preset_guid.text())
            .then_with(|| a.container.cmp(&b.container))
            .then_with(|| a.system.cmp(&b.system))
            .then_with(|| {
                let text = |location: &HarvestableLocation| {
                    location.starmap_object_guid.map(CatalogGuid::text)
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
mod tests {
    use super::*;

    fn guid(number: u8) -> Guid {
        format!("00000000-0000-0000-0000-0000000000{number:02x}")
            .parse()
            .expect("a GUID")
    }

    fn objects(entries: &[(u8, bool, Option<u8>)]) -> Objects {
        Objects {
            by_guid: entries
                .iter()
                .map(|&(own, claim, above)| (guid(own), (claim, above.map(guid))))
                .collect(),
        }
    }

    #[test]
    fn a_plain_object_is_the_location() {
        let objects = objects(&[(1, false, None)]);
        assert_eq!(objects.location(Some(guid(1))), Ok(CatalogGuid(guid(1))));
    }

    #[test]
    fn a_marker_gives_way_to_the_object_above() {
        let objects = objects(&[(1, false, None), (2, true, Some(1))]);
        assert_eq!(objects.location(Some(guid(2))), Ok(CatalogGuid(guid(1))));
    }

    #[test]
    fn each_missing_object_says_why() {
        let objects = objects(&[(2, true, None)]);
        assert_eq!(objects.location(None), Err(NoObject::NoneAbove));
        assert_eq!(objects.location(Some(guid(9))), Err(NoObject::NotInCatalog));
        assert_eq!(
            objects.location(Some(guid(2))),
            Err(NoObject::MarkerWithoutParent)
        );
    }

    /// `count` markers, each hanging below the next, then a plain object.
    fn markers_then_plain(count: u8) -> Objects {
        let entries: Vec<(u8, bool, Option<u8>)> = (1..=count)
            .map(|number| (number, true, Some(number + 1)))
            .chain([(count + 1, false, None)])
            .collect();
        objects(&entries)
    }

    #[test]
    fn sixty_three_markers_still_reach_the_object_above() {
        let objects = markers_then_plain(63);
        assert_eq!(objects.location(Some(guid(1))), Ok(CatalogGuid(guid(64))));
    }

    #[test]
    fn sixty_four_markers_are_too_long() {
        let objects = markers_then_plain(64);
        assert_eq!(objects.location(Some(guid(1))), Err(NoObject::ChainTooLong));
        assert_eq!(objects.location(Some(guid(2))), Ok(CatalogGuid(guid(65))));
    }

    #[test]
    fn a_container_with_no_object_above_is_no_loss() {
        assert!(!NoObject::NoneAbove.is_loss());
        assert!(NoObject::NotInCatalog.is_loss());
    }

    #[test]
    fn the_losses_count_every_reason_but_none_above() {
        let row = |reason| {
            (
                HarvestableLocation {
                    preset_guid: CatalogGuid(guid(1)),
                    starmap_object_guid: None,
                    system: String::new(),
                    container: String::new(),
                },
                reason,
            )
        };
        let rows = [
            row(None),
            row(Some(NoObject::NoneAbove)),
            row(Some(NoObject::NoneAbove)),
            row(Some(NoObject::NotInCatalog)),
            row(Some(NoObject::NotInCatalog)),
            row(Some(NoObject::MarkerWithoutParent)),
        ];
        assert_eq!(
            losses(&rows),
            vec![
                (NoObject::NotInCatalog, 2),
                (NoObject::MarkerWithoutParent, 1)
            ]
        );
    }
}
