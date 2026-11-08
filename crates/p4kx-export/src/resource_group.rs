// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Guid, RecordTree, Value};

use crate::guid_text::CatalogGuid;

/// Selected direct memberships and the physical authored selectors that establish them.
#[derive(Debug)]
pub(crate) struct Groups {
    by_resource: HashMap<Guid, CatalogGuid>,
    all_by_resource: HashMap<Guid, Vec<ResourceMembership>>,
    parents: HashMap<Guid, CatalogGuid>,
    complete: bool,
}

impl Default for Groups {
    fn default() -> Self {
        Self {
            by_resource: HashMap::new(),
            all_by_resource: HashMap::new(),
            parents: HashMap::new(),
            complete: true,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MembershipSource {
    pub(crate) record_index: u32,
    pub(crate) position: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ResourceMembership {
    pub(crate) group: CatalogGuid,
    pub(crate) sources: Vec<MembershipSource>,
}

pub(crate) struct ResourceMemberships {
    pub(crate) members: Vec<ResourceMembership>,
    pub(crate) complete: bool,
}

impl Groups {
    pub(crate) fn add(&mut self, listing: Listing) {
        self.complete &= listing.complete;
        let group = CatalogGuid(listing.group);
        for (position, resource) in listing.resources {
            self.by_resource.entry(resource).or_insert(group);
            let memberships = self.all_by_resource.entry(resource).or_default();
            let source = MembershipSource {
                record_index: listing.record_index,
                position,
            };
            if let Some(member) = memberships.iter_mut().find(|member| member.group == group) {
                member.sources.push(source);
            } else {
                memberships.push(ResourceMembership {
                    group,
                    sources: vec![source],
                });
            }
        }
        for (_, child) in listing.children {
            if child != listing.group {
                self.parents.entry(child).or_insert(group);
            }
        }
    }

    pub(crate) const fn mark_incomplete(&mut self) {
        self.complete = false;
    }

    pub(crate) fn of_resource(&self, resource: &Guid) -> Option<CatalogGuid> {
        self.by_resource.get(resource).copied()
    }

    pub(crate) fn all_of_resource(&self, resource: &Guid) -> ResourceMemberships {
        let mut members = self
            .all_by_resource
            .get(resource)
            .cloned()
            .unwrap_or_default();
        members.sort_by_cached_key(|member| member.group.text());
        ResourceMemberships {
            members,
            complete: self.complete,
        }
    }

    pub(crate) fn parent_of(&self, group: &Guid) -> Option<CatalogGuid> {
        self.parents.get(group).copied()
    }
}

/// One selected group with its actual Record and ordered member positions.
#[derive(Debug)]
pub(crate) struct Listing {
    group: Guid,
    record_index: u32,
    resources: Vec<(usize, Guid)>,
    children: Vec<(usize, Guid)>,
    complete: bool,
}

pub(crate) fn listing(tree: &RecordTree<'_>) -> Listing {
    let (resources, complete) = listed(tree, "resources");
    let (children, _) = listed(tree, "groups");
    Listing {
        group: tree.guid(),
        record_index: tree.record_index(),
        resources,
        children,
        complete,
    }
}

fn listed(tree: &RecordTree<'_>, field: &str) -> (Vec<(usize, Guid)>, bool) {
    let values = match tree.root().field(field) {
        None | Some(Value::Null) => return (Vec::new(), true),
        Some(Value::Array(values)) => values,
        _ => return (Vec::new(), false),
    };
    let mut complete = true;
    let mut listed = Vec::new();
    for (position, value) in values.iter().enumerate() {
        match value {
            Value::Reference(entry) if entry.location().is_none() => {
                listed.push((position, entry.guid()));
            }
            Value::Null => {}
            _ => complete = false,
        }
    }
    (listed, complete)
}
