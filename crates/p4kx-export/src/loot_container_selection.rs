// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_extraction::Guid;

use crate::guid_text::{CatalogGuid, GuidText};
use crate::harvest_locations::{NoObject, Objects};
use crate::loot_walk::{Placed, Summary, below};
use crate::starmap_walk::Opened;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ContainerKey<'source> {
    config: GuidText,
    container: &'source str,
    system: &'source str,
    object: Option<GuidText>,
}

impl<'source> ContainerKey<'source> {
    pub(super) fn new(
        config: Guid,
        container: &'source str,
        system: &'source str,
        object: Option<CatalogGuid>,
    ) -> Self {
        Self {
            config: CatalogGuid(config).text(),
            container,
            system,
            object: object.map(CatalogGuid::text),
        }
    }
}

pub(super) struct SelectedPlacement<'source> {
    pub(super) root: &'source Opened,
    pub(super) placed: &'source Placed,
    pub(super) location: Result<CatalogGuid, NoObject>,
}

impl<'source> SelectedPlacement<'source> {
    pub(super) fn key(&self) -> ContainerKey<'source> {
        ContainerKey::new(
            self.placed.config,
            &self.placed.path,
            &self.root.system,
            self.location.ok(),
        )
    }
}

/// Selects the first physical placement of each key in canonical traversal order.
pub(super) fn first_placements<'source>(
    opened: &'source [Opened],
    objects: &Objects<'_>,
    summaries: &'source HashMap<usize, Summary>,
) -> impl Iterator<Item = SelectedPlacement<'source>> + use<'source> {
    let mut reached: HashMap<usize, Vec<&Placed>> = HashMap::new();
    let mut selected = BTreeMap::new();
    for root in opened {
        let location = objects.location(root.under);
        let reached_placements = reached
            .entry(root.entry)
            .or_insert_with(|| below(root.entry, summaries));
        for &placed in reached_placements.as_slice() {
            let candidate = SelectedPlacement {
                root,
                placed,
                location,
            };
            selected.entry(candidate.key()).or_insert(candidate);
        }
    }
    selected.into_values()
}
