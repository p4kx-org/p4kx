// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Guid, RecordTree, Value};

use crate::guid_text::CatalogGuid;
use crate::record_fields::elements;

/// What the `ResourceTypeGroup` Records of the `resourceGroups` kind list: the group of each
/// `ResourceType` and the direct parent of each group.
///
/// Only the listings of the groups the kind selects are added, so every group it names is an entry
/// of `resourceGroups`. The parents form the lists as the game stores them; a cycle is kept, not
/// broken.
#[derive(Debug, Default)]
pub(crate) struct Groups {
    by_resource: HashMap<Guid, CatalogGuid>,
    all_by_resource: HashMap<Guid, Vec<CatalogGuid>>,
    parents: HashMap<Guid, CatalogGuid>,
}

impl Groups {
    /// Records what `listing` lists; the first group to list a `ResourceType` or a group wins.
    pub(crate) fn add(&mut self, listing: Listing) {
        let group = CatalogGuid(listing.group);
        for resource in listing.resources {
            self.by_resource.entry(resource).or_insert(group);
            let memberships = self.all_by_resource.entry(resource).or_default();
            if !memberships.contains(&group) {
                memberships.push(group);
            }
        }
        for child in listing.children {
            if child != listing.group {
                self.parents.entry(child).or_insert(group);
            }
        }
    }

    /// Returns the group that directly lists the `ResourceType` `resource`.
    pub(crate) fn of_resource(&self, resource: &Guid) -> Option<CatalogGuid> {
        self.by_resource.get(resource).copied()
    }

    /// Returns every selected group that directly lists the resource, distinct and GUID-ordered.
    pub(crate) fn all_of_resource(&self, resource: &Guid) -> Vec<CatalogGuid> {
        let mut groups = self
            .all_by_resource
            .get(resource)
            .cloned()
            .unwrap_or_default();
        groups.sort_by_cached_key(|group| group.text());
        groups
    }

    /// Returns the group that lists the group `group`.
    pub(crate) fn parent_of(&self, group: &Guid) -> Option<CatalogGuid> {
        self.parents.get(group).copied()
    }
}

/// The `ResourceType` Records and the groups one `ResourceTypeGroup` lists.
#[derive(Debug)]
pub(crate) struct Listing {
    group: Guid,
    resources: Vec<Guid>,
    children: Vec<Guid>,
}

/// Returns what the `ResourceTypeGroup` tree `tree` lists in `resources` and `groups`.
pub(crate) fn listing(tree: &RecordTree<'_>) -> Listing {
    Listing {
        group: tree.guid(),
        resources: listed(tree, "resources"),
        children: listed(tree, "groups"),
    }
}

fn listed(tree: &RecordTree<'_>, field: &str) -> Vec<Guid> {
    elements(Some(tree.root()), field)
        .iter()
        .filter_map(|entry| match entry {
            Value::Reference(entry) if entry.location().is_none() => Some(entry.guid()),
            _ => None,
        })
        .collect()
}
