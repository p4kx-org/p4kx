// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{CancelToken, Guid, RecordTree, SourceUseRole, Value};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;

#[path = "resource_group_parent_index.rs"]
mod parent_index;
#[path = "resource_group_parent_source.rs"]
mod parent_source;

pub(crate) use parent_index::ParentChoice;
use parent_index::Parents;
pub(crate) use parent_source::ParentSource;

use crate::guid_text::CatalogGuid;

/// Selected direct memberships and the physical authored selectors that establish them.
#[derive(Debug)]
pub(crate) struct Groups {
    by_resource: HashMap<Guid, CatalogGuid>,
    all_by_resource: HashMap<Guid, Vec<ResourceMembership>>,
    parents: Parents,
    selection_sources: Vec<GroupSelectionSource>,
    complete: bool,
}

impl Default for Groups {
    fn default() -> Self {
        Self {
            by_resource: HashMap::new(),
            all_by_resource: HashMap::new(),
            parents: Parents::default(),
            selection_sources: Vec::new(),
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

/// A group-array selector with actual read status independent of its header.
#[derive(Debug)]
pub(crate) struct GroupSelectionSource {
    pub(crate) record_index: u32,
    pub(crate) resources_role: SourceUseRole,
    prior_complete: bool,
}
/// The original first-group decision without sorting its authored input order.
pub(crate) struct ResourceGroupChoice<'groups> {
    pub(crate) group: Option<CatalogGuid>,
    pub(crate) selected: Option<&'groups MembershipSource>,
    selectors: &'groups [GroupSelectionSource],
    pub(crate) complete: bool,
}
impl ResourceGroupChoice<'_> {
    pub(crate) fn selectors(&self) -> impl Iterator<Item = &GroupSelectionSource> {
        let stop = self.selected.map(|source| source.record_index);
        self.selectors.iter().scan(false, move |finished, source| {
            if *finished {
                return None;
            }
            *finished = Some(source.record_index) == stop;
            Some(source)
        })
    }
}
impl Groups {
    pub(crate) fn add(
        &mut self,
        listing: Listing,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let prior_complete = self.complete;
        self.complete &= listing.complete;
        let group = CatalogGuid(listing.group);
        self.selection_sources.push(GroupSelectionSource {
            record_index: listing.record_index,
            resources_role: listing.resources_role,
            prior_complete,
        });
        for (position, resource) in listing.resources {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
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
        self.parents
            .add(listing.parent_source, listing.children, cancel)?;
        Ok(())
    }

    pub(crate) const fn mark_incomplete(&mut self) {
        self.complete = false;
    }

    pub(crate) fn of_resource(&self, resource: &Guid) -> Option<CatalogGuid> {
        self.by_resource.get(resource).copied()
    }

    pub(crate) fn first_of_resource(&self, resource: &Guid) -> ResourceGroupChoice<'_> {
        let group = self.of_resource(resource);
        let selected = group.and_then(|group| {
            self.all_by_resource
                .get(resource)
                .and_then(|members| members.iter().find(|member| member.group == group))
                .and_then(|member| member.sources.first())
        });
        let complete = selected.map_or(self.complete, |selected| {
            self.selection_sources
                .iter()
                .find(|source| source.record_index == selected.record_index)
                .is_some_and(|source| source.prior_complete)
        });
        ResourceGroupChoice {
            group,
            selected,
            selectors: &self.selection_sources,
            complete,
        }
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

    pub(crate) fn parent_choice(
        &self,
        group: &Guid,
        cancel: Option<&CancelToken>,
    ) -> Result<ParentChoice<'_>, CatalogError> {
        self.parents.choice(group, cancel)
    }

    pub(crate) fn parent_failure(&mut self, source: ParentSource) {
        self.parents.failed(source);
    }
}

/// One selected group with its actual Record and ordered member positions.
#[derive(Debug)]
pub(crate) struct Listing {
    group: Guid,
    record_index: u32,
    resources: Vec<(usize, Guid)>,
    children: Vec<(usize, Guid)>,
    parent_source: ParentSource,
    resources_role: SourceUseRole,
    complete: bool,
}

pub(crate) fn listing(
    tree: &RecordTree<'_>,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<Listing, CatalogError> {
    let (resources, complete) = listed(tree, "resources", cancel)?;
    let (parent_source, children) = ParentSource::read(tree, origins, cancel)?;
    Ok(Listing {
        group: tree.guid(),
        record_index: tree.record_index(),
        resources,
        children,
        parent_source,
        resources_role: if matches!(tree.field("resources"), Some(Value::Error(_))) {
            SourceUseRole::Attempted
        } else {
            SourceUseRole::Selector
        },
        complete,
    })
}

fn listed(
    tree: &RecordTree<'_>,
    field: &str,
    cancel: Option<&CancelToken>,
) -> Result<(Vec<(usize, Guid)>, bool), CatalogError> {
    let values = match tree.root().field(field) {
        None | Some(Value::Null) => return Ok((Vec::new(), true)),
        Some(Value::Array(values)) => values,
        _ => return Ok((Vec::new(), false)),
    };
    let mut complete = true;
    let mut listed = Vec::new();
    for (position, value) in values.iter().enumerate() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        match value {
            Value::Reference(entry) if entry.location().is_none() => {
                listed.push((position, entry.guid()));
            }
            Value::Null => {}
            _ => complete = false,
        }
    }
    Ok((listed, complete))
}
