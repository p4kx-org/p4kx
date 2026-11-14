// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use p4kx_extraction::{
    PhysicalSourceRef, SourceUse, SourceUses, SourceValue, SourcedValue, archive_animation_path,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_resources::{
    Resource, ResourceCapture, ResourceKind, Resources, resource_path,
};
use crate::soc_scene_xml::XmlDocuments;

use super::motion_options::{BranchOption, Database};

#[path = "player_subadb_link.rs"]
mod link;

use crate::player_source::{PlayerSourceError, check};
pub(super) use link::SubDatabase;
use link::{DatabaseLink, Visit};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DatabaseGraph {
    root: SourcedValue<usize>,
    branches: Vec<Branch>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Branch {
    document: SourcedValue<usize>,
    physical_source: SourcedValue<PhysicalSourceRef>,
    fragment_definition: SourcedValue<String>,
    tag_definition: SourcedValue<String>,
    lossy_xml: SourcedValue<bool>,
    options: Vec<BranchOption>,
    links: Vec<DatabaseLink>,
}

struct Frame {
    document: usize,
    links: std::vec::IntoIter<SubDatabase>,
    uses: Vec<SourceUse>,
}

struct Graph<'a, 'archive, 'sink> {
    resources: &'a Resources<'archive>,
    documents: &'a mut XmlDocuments<'sink>,
    captures: HashMap<String, ResourceCapture<usize>>,
    visits: HashMap<usize, usize>,
    branches: Vec<Branch>,
    active: Vec<Frame>,
}

pub(super) struct DatabaseRoot<'a> {
    pub(super) database: &'a mut Database,
    pub(super) document: usize,
    pub(super) authored_path: &'a str,
    pub(super) selector: &'a SourceUse,
    pub(super) capture: ResourceCapture<usize>,
}

struct Lookup {
    target: SourcedValue<Resource<usize>>,
    uses: Vec<SourceUse>,
    next: Option<Frame>,
}

impl DatabaseGraph {
    pub(super) fn read(
        resources: &Resources<'_>,
        root: DatabaseRoot<'_>,
        documents: &mut XmlDocuments<'_>,
    ) -> Result<Self, PlayerSourceError> {
        let DatabaseRoot {
            database,
            document,
            authored_path,
            selector,
            capture,
        } = root;
        let root_path = resource_path(authored_path, ResourceKind::DataRelative)
            .ok_or(SocSceneError::Position)?;
        let root_key = root_path.to_ascii_lowercase();
        let mut graph = Graph {
            resources,
            documents,
            captures: HashMap::from([(root_key.clone(), capture)]),
            visits: HashMap::from([(document, 0)]),
            branches: vec![Branch::of(resources.sources, database, document)],
            active: vec![Frame {
                document,
                links: std::mem::take(&mut database.links).into_iter(),
                uses: Vec::new(),
            }],
        };
        while !graph.active.is_empty() {
            check(resources.cancel)?;
            graph.step()?;
        }
        let capture = graph
            .captures
            .get(&root_key)
            .ok_or(SocSceneError::Position)?;
        let mut uses = vec![selector.clone()];
        uses.extend(capture.uses().iter().cloned());
        let root = resources
            .sources
            .value(SourceValue::Present(document), &uses, true);
        Ok(Self {
            root,
            branches: graph.branches,
        })
    }
}

impl Graph<'_, '_, '_> {
    fn step(&mut self) -> Result<(), SocSceneError> {
        let Some(frame) = self.active.last_mut() else {
            return Ok(());
        };
        let document = frame.document;
        let Some(link) = frame.links.next() else {
            self.active.pop();
            return Ok(());
        };
        let source = self
            .visits
            .get(&document)
            .copied()
            .ok_or(SocSceneError::Position)?;
        let (walked, next) = self.resolve(link)?;
        self.branches
            .get_mut(source)
            .ok_or(SocSceneError::Position)?
            .links
            .push(walked);
        self.active.extend(next);
        Ok(())
    }

    fn resolve(
        &mut self,
        link: SubDatabase,
    ) -> Result<(DatabaseLink, Option<Frame>), SocSceneError> {
        let Lookup {
            target,
            mut uses,
            next,
        } = match link.path() {
            Some(path) => self.lookup(&path, &link)?,
            None => Lookup {
                target: link.without_target(self.resources.sources),
                uses: Vec::new(),
                next: None,
            },
        };
        let visit = match target.value() {
            Some(Resource::Present { data, .. }) => self.visit(*data, &mut uses),
            _ => Visit::Unresolved,
        };
        let traversal = link.visit(
            self.resources.sources,
            visit,
            uses,
            target.origin().source_set_complete(),
        );
        Ok((link.project(target, traversal), next))
    }

    fn lookup(&mut self, path: &str, link: &SubDatabase) -> Result<Lookup, SocSceneError> {
        let key = path.to_ascii_lowercase();
        let mut next = None;
        if let Entry::Vacant(slot) = self.captures.entry(key.clone()) {
            let mut loaded = None;
            let capture = self.resources.capture_with_documents(
                SourceValue::Present(path.to_owned()),
                ResourceKind::DataRelative,
                self.documents,
                |document, origin, documents| {
                    if document.root().name() != "AnimDB" || document.root().namespace().is_some() {
                        return Ok(None);
                    }
                    let draft = documents.adb_declaration(origin);
                    let mut database =
                        Database::read(document, origin, self.resources.cancel, draft)?;
                    let index = documents.bind(document, origin)?;
                    if let Some(seeds) = documents.tag_definitions() {
                        seeds.owner(index, document.root(), origin)?;
                    }
                    database.retain_declarations(documents, index);
                    loaded = Some((index, database));
                    Ok(Some(index))
                },
            )?;
            if let Some((document, database)) = loaded
                && let Entry::Vacant(branch) = self.visits.entry(document)
            {
                branch.insert(self.branches.len());
                self.branches
                    .push(Branch::of(self.resources.sources, &database, document));
                let mut uses = link.selectors().to_vec();
                uses.extend(capture.uses().iter().cloned());
                next = Some(Frame {
                    document,
                    links: database.links.into_iter(),
                    uses,
                });
            }
            slot.insert(capture);
        }
        let capture = self.captures.get(&key).ok_or(SocSceneError::Position)?;
        Ok(Lookup {
            target: capture.bind(self.resources.sources, link.selectors()),
            uses: capture.uses().to_vec(),
            next,
        })
    }

    fn visit(&self, target: usize, proof: &mut Vec<SourceUse>) -> Visit {
        let Some(index) = self
            .active
            .iter()
            .position(|frame| frame.document == target)
        else {
            return Visit::Reachable;
        };
        proof.extend(
            self.active
                .iter()
                .skip(index + 1)
                .flat_map(|frame| frame.uses.iter().cloned()),
        );
        Visit::Cycle
    }
}

impl Branch {
    fn of(sources: &SourceUses, database: &Database, document: usize) -> Self {
        let root = std::slice::from_ref(&database.root_use);
        Self {
            document: sources.value(SourceValue::Present(document), root, true),
            physical_source: sources.value(
                SourceValue::Present(database.source.clone()),
                root,
                true,
            ),
            fragment_definition: database.file.value.clone(),
            tag_definition: database.tag_file.clone(),
            lossy_xml: sources.value(SourceValue::Present(database.lossy), root, true),
            options: database.branch_options(sources),
            links: Vec::new(),
        }
    }
}

fn explicit_path(authored: &str) -> Option<String> {
    let (_, extension) = authored.rsplit_once('.')?;
    if extension.eq_ignore_ascii_case("adb") {
        archive_animation_path(authored)
    } else {
        None
    }
}
