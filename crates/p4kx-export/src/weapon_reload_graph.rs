// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{CancelToken, SourceLocation, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::{PhysicalEntryRef, SourceXmlAttribute};
use crate::schema_fields::every_field_required;

use super::assets::{AssetLink, Assets, FileSelector};
use super::document::{Branch, check_cancel};

#[derive(Debug, Copy, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(super) enum BranchVisit {
    Resolved,
    Unresolved,
    Cycle,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct BranchLink {
    pub(super) source: PhysicalEntryRef,
    pub(super) position: SourcedValue<SourceLocation>,
    pub(super) target: AssetLink,
    traversal: SourcedValue<BranchVisit>,
    pub(super) fragment_selectors: Vec<SourcedValue<String>>,
    pub(super) tags: SourcedValue<String>,
}

impl BranchLink {
    pub(super) fn visit(&self) -> Option<BranchVisit> {
        self.traversal
            .value()
            .copied()
            .filter(|_| self.traversal.origin().source_set_complete())
    }

    pub(super) const fn is_resolved(&self) -> bool {
        matches!(self.traversal.value(), Some(BranchVisit::Resolved))
    }
}

struct Pending {
    position: SourcedValue<SourceLocation>,
    path: SourcedValue<String>,
    tags: SourcedValue<String>,
    fragments: Vec<SourcedValue<String>>,
}

struct Walk {
    source: PhysicalEntryRef,
    edges: std::vec::IntoIter<Pending>,
}

pub(super) fn walk(
    assets: &mut Assets<'_, '_, '_>,
    roots: impl IntoIterator<Item = PhysicalEntryRef>,
    sources: &SourceUses,
    cancel: Option<&CancelToken>,
) -> Result<Vec<BranchLink>, CatalogError> {
    let mut graph = WalkGraph {
        assets,
        rows: Vec::new(),
        seen: BTreeSet::new(),
        active: BTreeSet::new(),
        stack: Vec::new(),
        sources,
    };
    for root in roots {
        graph.enter(root)?;
        while !graph.stack.is_empty() {
            check_cancel(cancel)?;
            graph.step()?;
        }
    }
    Ok(graph.rows)
}

struct WalkGraph<'call, 'archive, 'capture, 'input> {
    assets: &'call mut Assets<'archive, 'capture, 'input>,
    rows: Vec<BranchLink>,
    seen: BTreeSet<PhysicalEntryRef>,
    active: BTreeSet<PhysicalEntryRef>,
    stack: Vec<Walk>,
    sources: &'call SourceUses,
}

impl WalkGraph<'_, '_, '_, '_> {
    fn enter(&mut self, source: PhysicalEntryRef) -> Result<(), CatalogError> {
        let Some(database) = self.assets.databases.get(&source) else {
            return Ok(());
        };
        if !self.seen.insert(source.clone()) {
            return Ok(());
        }
        let edges = database
            .branches
            .iter()
            .map(|branch| Pending::read(branch, self.sources))
            .collect::<Result<Vec<_>, _>>()?;
        self.active.insert(source.clone());
        self.stack.push(Walk {
            source,
            edges: edges.into_iter(),
        });
        Ok(())
    }

    fn step(&mut self) -> Result<(), CatalogError> {
        let Some(walk) = self.stack.last_mut() else {
            return Ok(());
        };
        let Some(edge) = walk.edges.next() else {
            self.active.remove(&walk.source);
            self.stack.pop();
            return Ok(());
        };
        let source = walk.source.clone();
        let target = self.assets.select(&edge.path, FileSelector::SubDatabase)?;
        let target_id = target
            .source
            .clone()
            .filter(|entry| self.assets.databases.contains_key(entry));
        let visit = match target_id.as_ref() {
            Some(child) if self.active.contains(child) => BranchVisit::Cycle,
            Some(_) => BranchVisit::Resolved,
            None => BranchVisit::Unresolved,
        };
        let traversal = self.sources.value_with_origins(
            SourceValue::Present(visit),
            &[edge.position.origin(), target.origin()],
        )?;
        self.rows.push(BranchLink {
            source,
            position: edge.position,
            tags: edge.tags,
            fragment_selectors: edge.fragments,
            target,
            traversal,
        });
        if let Some(child) = target_id {
            self.enter(child)?;
        }
        Ok(())
    }
}

impl Pending {
    fn read(branch: &Branch, sources: &SourceUses) -> Result<Self, CatalogError> {
        let position = branch.position().clone();
        let path = attribute(&branch.attributes, "File", &position, sources)?;
        let tags = attribute(&branch.attributes, "Tags", &position, sources)?;
        let fragments = branch
            .fragments
            .iter()
            .map(|fragment| attribute(&fragment.attributes, "Name", fragment.position(), sources))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            position,
            path,
            tags,
            fragments,
        })
    }
}

pub(super) fn attribute(
    attributes: &[SourceXmlAttribute],
    name: &str,
    position: &SourcedValue<SourceLocation>,
    sources: &SourceUses,
) -> Result<SourcedValue<String>, CatalogError> {
    let selected = attributes
        .iter()
        .filter(|attribute| {
            attribute.name.value().is_some_and(|value| value == name)
                && attribute.namespace.value().is_some_and(Option::is_none)
        })
        .collect::<Vec<_>>();
    match selected.as_slice() {
        [] => Ok(sources.value_with_origins(SourceValue::Absent, &[position.origin()])?),
        [original] => Ok(original.value.clone()),
        _ => {
            let origins = selected
                .iter()
                .map(|attribute| attribute.value.origin())
                .collect::<Vec<_>>();
            Ok(sources.value_with_origins(SourceValue::Invalid, &origins)?)
        }
    }
}

pub(super) fn by_database(edges: &[BranchLink]) -> BTreeMap<&PhysicalEntryRef, Vec<&BranchLink>> {
    let mut owners = BTreeMap::<_, Vec<_>>::new();
    for edge in edges {
        owners.entry(&edge.source).or_default().push(edge);
    }
    owners
}
