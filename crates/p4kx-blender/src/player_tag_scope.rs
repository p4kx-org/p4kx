// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use p4kx_extraction::{CancelToken, FieldOrigin, SourceUse, SourceUses, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_resources::Resource;
use crate::soc_scene_origin::SocXmlPosition;

use super::source::{self, Kind, Role};
use super::{Document, Edge};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagScope {
    kind: Kind,
    documents: Vec<usize>,
    links: Vec<Link>,
    #[serde(flatten)]
    pub(super) origin: FieldOrigin,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(super) enum Link {
    Root { index: usize },
    Definition { document: usize, index: usize },
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct Candidate {
    pub(super) document: usize,
    position: SourcedValue<SocXmlPosition>,
    name: SourcedValue<String>,
}

pub(super) struct Survey {
    pub(super) output: TagScope,
    names: BTreeMap<String, Vec<Candidate>>,
}

struct Traversal<'a> {
    output: Vec<usize>,
    links: Vec<Link>,
    pending: VecDeque<usize>,
    visited: BTreeSet<usize>,
    uses: Vec<SourceUse>,
    names: BTreeMap<String, Vec<Candidate>>,
    complete: bool,
    cancel: &'a CancelToken,
}

impl Survey {
    pub(super) fn read(
        kind: Kind,
        documents: &BTreeMap<usize, &Document>,
        start: (Link, &Edge),
        sources: &SourceUses,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let mut traversal = Traversal {
            output: Vec::new(),
            links: Vec::new(),
            pending: VecDeque::new(),
            visited: BTreeSet::new(),
            uses: Vec::new(),
            names: BTreeMap::new(),
            complete: true,
            cancel,
        };
        traversal.follow(start.0, start.1);
        while let Some(index) = traversal.pending.pop_front() {
            let document = documents.get(&index).ok_or(SocSceneError::Position)?;
            traversal.read(kind, document)?;
        }
        Ok(Self {
            output: TagScope {
                kind,
                documents: traversal.output,
                links: traversal.links,
                origin: sources.origin(&traversal.uses, traversal.complete),
            },
            names: traversal.names,
        })
    }

    pub(super) fn candidates(&self, spelling: &str) -> &[Candidate] {
        self.names
            .get(&spelling.to_ascii_lowercase())
            .map_or(&[], Vec::as_slice)
    }
}

impl Traversal<'_> {
    fn read(&mut self, kind: Kind, document: &Document) -> Result<(), SocSceneError> {
        source::check(self.cancel)?;
        if !document.kinds.contains(&kind) {
            return Err(SocSceneError::Position);
        }
        self.output.push(document.index);
        let definition = &document.declaration;
        self.uses.extend_from_slice(&definition.evidence);
        self.complete &= definition
            .version
            .value()
            .value()
            .is_some_and(|value| value == "2");
        for tag in definition.tags() {
            source::check(self.cancel)?;
            self.uses.extend_from_slice(&tag.name_uses);
            let value = tag.name.value();
            let Some(name) = value.value().filter(|name| !name.is_empty()) else {
                self.complete = false;
                continue;
            };
            self.names
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push(Candidate {
                    document: document.index,
                    position: tag.position.clone(),
                    name: value.clone(),
                });
        }
        for (index, edge) in document.links.iter().enumerate() {
            source::check(self.cancel)?;
            if matches!(edge.role, Role::Import) && edge.owner_kind == Some(kind) {
                self.follow(
                    Link::Definition {
                        document: document.index,
                        index,
                    },
                    edge,
                );
            }
        }
        Ok(())
    }

    fn follow(&mut self, link: Link, edge: &Edge) {
        self.links.push(link);
        self.uses.extend_from_slice(&edge.evidence);
        if let Some(Resource::Present { data, .. }) = edge.target.value() {
            if self.visited.insert(*data) {
                self.pending.push_back(*data);
            }
        } else {
            self.complete = false;
        }
    }
}

impl Candidate {
    pub(super) fn owns(&self, edge: &Edge) -> bool {
        matches!(edge.role, Role::FragmentTags) && edge.position.value() == self.position.value()
    }
}
