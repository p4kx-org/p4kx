// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::{PhysicalSourceRef, SourceUse, SourceUseRole, SourceValue, SourcedValue};
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_resources::{self, Resource, ResourceCapture, ResourceKind, Resources};
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_xml::XmlDocuments;

#[path = "player_tag_definition_source.rs"]
mod source;

#[path = "player_tag_definition_cycles.rs"]
mod cycles;

use source::{Attribute, Definition, Kind, Owner, PendingDefinition, PendingEdge, Role};

#[derive(Default)]
pub(crate) struct Seeds {
    owners: Vec<Owner>,
    bound_owners: BTreeSet<usize>,
    captures: BTreeMap<String, ResourceCapture<usize>>,
    observed: BTreeMap<String, PendingDefinition>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagDefinitions {
    roots: Vec<Edge>,
    documents: Vec<Document>,
    cycle_back_edges: Vec<CycleEdge>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Edge {
    role: Role,
    owner: usize,
    owner_kind: Option<Kind>,
    target_kind: Kind,
    position: SourcedValue<SocXmlPosition>,
    selector: Attribute,
    target: SourcedValue<Resource<usize>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    #[serde(rename = "document")]
    index: usize,
    source: PhysicalSourceRef,
    kinds: BTreeSet<Kind>,
    declaration: Definition,
    links: Vec<Edge>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CycleEdge {
    owner: usize,
    kind: Kind,
    link: usize,
}

struct Traversal<'a, 'archive, 'sink> {
    resources: &'a Resources<'archive>,
    documents: &'a mut XmlDocuments<'sink>,
    observed: BTreeMap<String, PendingDefinition>,
    captures: BTreeMap<String, ResourceCapture<usize>>,
    pending: VecDeque<(usize, Kind)>,
    visited: BTreeSet<(usize, Kind)>,
    definitions: BTreeMap<usize, PendingDefinition>,
}

impl Seeds {
    pub(crate) fn owner(
        &mut self,
        document: usize,
        root: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<(), SocSceneError> {
        if self.bound_owners.insert(document) {
            let owner = match root.name() {
                "AnimDB" => Owner::database(document, root, origin)?,
                "ControllerDef" => Owner::controller(document, root, origin)?,
                _ => return Ok(()),
            };
            self.owners.push(owner);
        }
        Ok(())
    }

    pub(crate) fn finish(
        self,
        resources: &Resources<'_>,
        documents: &mut XmlDocuments<'_>,
    ) -> Result<TagDefinitions, SocSceneError> {
        let mut traversal = Traversal {
            resources,
            documents,
            observed: self.observed,
            captures: self.captures,
            pending: VecDeque::new(),
            visited: BTreeSet::new(),
            definitions: BTreeMap::new(),
        };
        let roots = traversal.roots(self.owners)?;
        let definitions = traversal.resolve()?;
        let cycle_back_edges = cycles::find(&definitions, resources.cancel)?;
        Ok(TagDefinitions {
            roots,
            documents: definitions,
            cycle_back_edges,
        })
    }

    pub(crate) fn observe_definition(
        &mut self,
        path: &str,
        index: usize,
        root: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
        cancel: &p4kx_extraction::CancelToken,
    ) -> Result<(), SocSceneError> {
        let key = path.to_ascii_lowercase();
        if let Entry::Vacant(slot) = self.observed.entry(key) {
            slot.insert(PendingDefinition::read(index, root, origin, cancel)?);
        }
        Ok(())
    }

    pub(crate) fn retain_lookup(&mut self, path: &str, capture: &ResourceCapture<usize>) {
        self.captures
            .entry(path.to_ascii_lowercase())
            .or_insert_with(|| capture.clone());
    }
}

impl Traversal<'_, '_, '_> {
    fn roots(&mut self, owners: Vec<Owner>) -> Result<Vec<Edge>, SocSceneError> {
        let mut roots = Vec::new();
        for owner in owners {
            roots.extend(self.edges(&owner, None)?);
        }
        Ok(roots)
    }

    fn resolve(&mut self) -> Result<Vec<Document>, SocSceneError> {
        let mut documents = Vec::<Document>::new();
        let mut positions = BTreeMap::<usize, usize>::new();
        while let Some((index, kind)) = self.pending.pop_front() {
            source::check(self.resources.cancel)?;
            let definition = self
                .definitions
                .get(&index)
                .ok_or(SocSceneError::Position)?;
            let declaration = if positions.contains_key(&index) {
                None
            } else {
                Some(definition.bind(self.resources.sources, self.resources.cancel)?)
            };
            let owner = definition.owner.clone();
            let links = self.edges(&owner, Some(kind))?;
            match positions.entry(index) {
                Entry::Occupied(position) => {
                    let output = documents
                        .get_mut(*position.get())
                        .ok_or(SocSceneError::Position)?;
                    output.kinds.insert(kind);
                    output.links.extend(links);
                }
                Entry::Vacant(position) => {
                    let declaration = declaration.ok_or(SocSceneError::Position)?;
                    position.insert(documents.len());
                    documents.push(Document {
                        index,
                        source: owner.scope.source_ref().clone(),
                        kinds: BTreeSet::from([kind]),
                        declaration,
                        links,
                    });
                }
            }
        }
        Ok(documents)
    }

    fn edges(&mut self, owner: &Owner, kind: Option<Kind>) -> Result<Vec<Edge>, SocSceneError> {
        owner
            .edges
            .iter()
            .filter(|edge| {
                !matches!(edge.role, Role::FragmentTags) || kind == Some(Kind::Fragments)
            })
            .map(|edge| self.edge(owner, edge, kind))
            .collect()
    }

    fn edge(
        &mut self,
        owner: &Owner,
        edge: &PendingEdge,
        kind: Option<Kind>,
    ) -> Result<Edge, SocSceneError> {
        source::check(self.resources.cancel)?;
        let target_kind = edge.role.kind(kind).ok_or(SocSceneError::Position)?;
        let origin = source::origin(self.resources.sources, &owner.scope);
        let (position, node) = source::node(&origin, edge.position.clone())?;
        let (selector, selectors) = edge
            .selector
            .bind(&origin, &node, SourceUseRole::Selector)?;
        let target = self.target(edge.selector.value(), &selectors)?;
        if let Some(Resource::Present { data, .. }) = target.value()
            && self.visited.insert((*data, target_kind))
        {
            self.pending.push_back((*data, target_kind));
        }
        Ok(Edge {
            role: edge.role,
            owner: owner.document,
            owner_kind: kind,
            target_kind,
            position,
            selector,
            target,
        })
    }

    fn target(
        &mut self,
        value: SourceValue<String>,
        selectors: &[SourceUse],
    ) -> Result<SourcedValue<Resource<usize>>, SocSceneError> {
        let key = match &value {
            SourceValue::Present(path) => {
                soc_scene_class_resources::resource_path(path, ResourceKind::DataRelative)
                    .map(|path| path.to_ascii_lowercase())
            }
            _ => None,
        };
        let Some(key) = key else {
            let capture = self.resources.capture_with_documents(
                value,
                ResourceKind::DataRelative,
                self.documents,
                |_, _, _| Ok(None),
            )?;
            return Ok(capture.into_bound(self.resources.sources, selectors));
        };
        if let Some(definition) = self.observed.remove(&key) {
            let original = self.captures.get_mut(&key).ok_or(SocSceneError::Position)?;
            if definition.known_root() {
                self.definitions
                    .entry(definition.owner.document)
                    .or_insert(definition);
            } else {
                let source = definition.owner.scope.source_ref().clone();
                *original = ResourceCapture::from_observation(
                    Resource::InvalidXml { source },
                    original.uses().to_vec(),
                );
            }
        }
        if let Entry::Vacant(slot) = self.captures.entry(key.clone()) {
            slot.insert(capture(
                value,
                self.resources,
                self.documents,
                &mut self.definitions,
            )?);
        }
        let capture = self.captures.get(&key).ok_or(SocSceneError::Position)?;
        Ok(capture.bind(self.resources.sources, selectors))
    }
}

fn capture(
    value: SourceValue<String>,
    resources: &Resources<'_>,
    documents: &mut XmlDocuments<'_>,
    definitions: &mut BTreeMap<usize, PendingDefinition>,
) -> Result<ResourceCapture<usize>, SocSceneError> {
    resources.capture_with_documents(
        value,
        ResourceKind::DataRelative,
        documents,
        |xml, origin, docs| {
            let root = xml.root();
            if root.namespace().is_some() || root.name() != "TagDefinition" {
                return Ok(None);
            }
            let index = docs.bind(xml, origin)?;
            if let Entry::Vacant(slot) = definitions.entry(index) {
                slot.insert(PendingDefinition::read(
                    index,
                    root,
                    origin,
                    resources.cancel,
                )?);
            }
            Ok(Some(index))
        },
    )
}
