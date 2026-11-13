// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_extraction::{Archive, ArchiveMaterialRead};
use p4kx_model::{ChildContainer, MaterialStatus, SocContainerDescriptor, Transform};

use super::child_container_path::ChildContainerPath;
use super::classes::ClassLookup;
use super::files::{find_first, path_key, stem};
use super::layers::Layers;
use super::scan::{Scan, scan_container};
use super::tile_light::{TileLightOwner, TileLightPlacement};
use super::{
    InteriorOptions, InteriorPart, InteriorPartKind, InteriorWatch, MAX_INTERIOR_CONTAINERS,
    MAX_INTERIOR_DEPTH,
};
use crate::{AssemblyError, Geometry, Issue, IssueKind, PaletteRef};

mod classes;
mod entities;
mod streams;

/// A model part waiting for its model: where to look for the file, outermost layer first.
pub(super) struct Placement {
    pub(super) part: usize,
    pub(super) chain: Arc<[usize]>,
    pub(super) container: Arc<str>,
    pub(super) library: Option<String>,
    pub(super) palette: Option<PaletteRef>,
}

/// What the walk found, before any model is read.
pub(super) struct Walked<'a> {
    pub(super) parts: Vec<InteriorPart>,
    pub(super) placements: Vec<Placement>,
    pub(super) issues: Vec<Issue>,
    pub(super) layers: Layers<'a>,
    pub(super) containers: usize,
}

struct Todo {
    layer: usize,
    chain: Arc<[usize]>,
    part: usize,
    path: Arc<str>,
    stem: String,
    ids: Vec<String>,
    depth: usize,
}

fn identity(layer: usize, path: &str) -> String {
    format!("{layer}|{}", path_key(path))
}

fn container_part(
    name: String,
    parent: Option<usize>,
    local: Transform,
    world: Transform,
) -> InteriorPart {
    InteriorPart {
        name,
        kind: InteriorPartKind::Container,
        parent,
        local,
        world,
        model_path: None,
        model: None,
        materials: Arc::new(ArchiveMaterialRead::unobserved(MaterialStatus::Unnamed)),
        palette: None,
        light: None,
        light_placement: None,
    }
}

/// Walks the containers of the Interior: the root container `layers.last()` that was opened from
/// the Entry `entry` of the layer below it, and every container nested in it.
pub(super) fn walk<'a>(
    given: &'a [&'a Archive],
    entry: &str,
    options: &InteriorOptions,
    watch: &InteriorWatch<'_>,
    classes: Option<&'a dyn ClassLookup>,
    owner: Option<TileLightOwner<'_>>,
) -> Result<Walked<'a>, AssemblyError> {
    let mut walker = Walker {
        layers: Layers::new(given),
        parts: Vec::new(),
        placements: Vec::new(),
        issues: Vec::new(),
        full: false,
        max_parts: options.max_parts,
        max_containers: options.max_containers.min(MAX_INTERIOR_CONTAINERS),
        classes,
        class_geometry: HashMap::new(),
        palette_paths: HashMap::new(),
    };
    let Some(root_layer) = given.len().checked_sub(1) else {
        return Ok(walker.finish());
    };
    let name = stem(entry).to_owned();
    walker.parts.push(container_part(
        name.clone(),
        None,
        Transform::IDENTITY,
        Transform::IDENTITY,
    ));
    let below = root_layer.checked_sub(1).unwrap_or(usize::MAX);
    let mut stack = vec![Todo {
        layer: root_layer,
        chain: (0..=root_layer).collect(),
        part: 0,
        path: name.as_str().into(),
        stem: name,
        ids: vec![identity(below, entry)],
        depth: 0,
    }];
    while let Some(todo) = stack.pop() {
        if watch.is_cancelled() {
            return Err(AssemblyError::Cancelled);
        }
        let direct_owner = owner.filter(|_| todo.depth == 0);
        let scan = walker.scan(&todo, direct_owner.is_some());
        if let Some(error) = scan.fatal {
            return Err(error);
        }
        walker.report(&todo, scan.issues);
        let descriptor = scan.descriptor.as_ref().and_then(|source| {
            let descriptor = SocContainerDescriptor::parse(&source.bytes).ok()?;
            (!descriptor.document().is_lossy()).then_some((source, descriptor))
        });
        let tile = direct_owner.map(|owner| {
            TileLightPlacement::new(
                owner,
                descriptor
                    .as_ref()
                    .map(|(source, descriptor)| (*source, descriptor)),
            )
        });
        walker.add_streams(&todo, scan.streams, tile.as_ref());
        let mut nested = walker.add_children(&todo, scan.children)?;
        nested.reverse();
        stack.extend(nested);
    }
    Ok(walker.finish())
}

struct Walker<'a> {
    layers: Layers<'a>,
    parts: Vec<InteriorPart>,
    placements: Vec<Placement>,
    issues: Vec<Issue>,
    full: bool,
    max_parts: usize,
    max_containers: usize,
    classes: Option<&'a dyn ClassLookup>,
    class_geometry: HashMap<String, Option<Arc<Geometry>>>,
    palette_paths: HashMap<String, Option<PaletteRef>>,
}

impl<'a> Walker<'a> {
    fn finish(self) -> Walked<'a> {
        let containers = self.layers.opened.len() + 1;
        Walked {
            parts: self.parts,
            placements: self.placements,
            issues: self.issues,
            layers: self.layers,
            containers,
        }
    }

    fn note(&mut self, path: &str, name: &str, kind: IssueKind) {
        self.issues.push(Issue {
            path: path.to_owned(),
            name: name.to_owned(),
            kind,
        });
    }

    fn report(&mut self, todo: &Todo, issues: Vec<IssueKind>) {
        for kind in issues {
            self.note(&todo.path, &todo.stem, kind);
        }
    }

    fn room(&mut self, todo: &Todo) -> bool {
        if self.parts.len() >= self.max_parts {
            if !self.full {
                self.full = true;
                let limit = self.max_parts;
                self.note(&todo.path, &todo.stem, IssueKind::TooManyParts { limit });
            }
            return false;
        }
        true
    }

    fn scan(&self, todo: &Todo, capture_tile: bool) -> Scan {
        let (Some(archive), Some(keys)) = (
            self.layers.get(todo.layer),
            self.layers.keys.get(todo.layer),
        ) else {
            return Scan {
                children: Vec::new(),
                streams: Vec::new(),
                issues: Vec::new(),
                fatal: None,
                descriptor: None,
            };
        };
        scan_container(archive, keys, &todo.stem, capture_tile)
    }

    fn add_children(
        &mut self,
        todo: &Todo,
        children: Vec<ChildContainer>,
    ) -> Result<Vec<Todo>, AssemblyError> {
        let mut nested = Vec::new();
        for child in children {
            let name = if child.name.is_empty() {
                child.entity_name.clone()
            } else {
                child.name.clone()
            };
            let Some(local) = child.transform().filter(|_| !child.malformed) else {
                let reason =
                    "pos and rot are no finite numbers or the rotation has no length".to_owned();
                self.note(
                    &todo.path,
                    &name,
                    IssueKind::InvalidPlacement {
                        name: name.clone(),
                        reason,
                    },
                );
                continue;
            };
            if todo.depth + 1 >= MAX_INTERIOR_DEPTH {
                self.note(
                    &todo.path,
                    &name,
                    IssueKind::TooDeep {
                        limit: MAX_INTERIOR_DEPTH,
                    },
                );
                continue;
            }
            if self.layers.opened.len() + 1 >= self.max_containers {
                let limit = self.max_containers;
                self.note(&todo.path, &name, IssueKind::TooManyContainers { limit });
                continue;
            }
            if !self.room(todo) {
                break;
            }
            if let Some(next) = self.enter(todo, &child, &name, local)? {
                nested.push(next);
            }
        }
        Ok(nested)
    }

    /// Finds the file of the nested container `child`, checks it is no ancestor and opens it;
    /// returns the container and its Entry path and identity.
    fn open_child(
        &mut self,
        todo: &Todo,
        child: &ChildContainer,
        name: &str,
    ) -> Result<Option<(Archive, String, String)>, AssemblyError> {
        let found = ChildContainerPath::new(&child.name).and_then(|path| {
            todo.chain.iter().rev().find_map(|&layer| {
                let archive = self.layers.get(layer)?;
                let keys = self.layers.keys.get(layer)?;
                let index = keys.index(archive);
                let entry = if layer == 0 {
                    find_first(&index, path.root_candidates())
                } else {
                    index.find(path.relative())
                };
                entry.map(|entry| (layer, entry))
            })
        });
        let Some((layer, entry)) = found else {
            let kind = IssueKind::ContainerMissing {
                name: child.name.clone(),
            };
            self.note(&todo.path, name, kind);
            return Ok(None);
        };
        let entry_path = entry.path().to_owned();
        let id = identity(layer, &entry_path);
        if todo.ids.contains(&id) {
            self.note(
                &todo.path,
                name,
                IssueKind::ContainerCycle {
                    name: child.name.clone(),
                },
            );
            return Ok(None);
        }
        let opened = self
            .layers
            .get(layer)
            .map(|archive| archive.open_container(entry));
        let archive = match opened {
            Some(Ok(archive)) => archive,
            Some(Err(error)) if error.is_key_required() || error.is_wrong_key() => {
                return Err(AssemblyError::Provider(Box::new(error)));
            }
            Some(Err(error)) => {
                let kind = IssueKind::ContainerUnreadable {
                    name: child.name.clone(),
                    reason: error.to_string(),
                };
                self.note(&todo.path, name, kind);
                return Ok(None);
            }
            None => return Ok(None),
        };
        Ok(Some((archive, entry_path, id)))
    }

    fn enter(
        &mut self,
        todo: &Todo,
        child: &ChildContainer,
        name: &str,
        local: Transform,
    ) -> Result<Option<Todo>, AssemblyError> {
        let Some((archive, entry_path, id)) = self.open_child(todo, child, name)? else {
            return Ok(None);
        };
        let layer = self.layers.push(archive);
        let world = self
            .parts
            .get(todo.part)
            .map_or(local, |parent| parent.world.then(&local));
        let part = self.parts.len();
        self.parts.push(container_part(
            stem(name).to_owned(),
            Some(todo.part),
            local,
            world,
        ));
        let mut chain = todo.chain.to_vec();
        chain.push(layer);
        let mut ids = todo.ids.clone();
        ids.push(id);
        Ok(Some(Todo {
            layer,
            chain: chain.into(),
            part,
            path: format!("{}/{}", todo.path, stem(name)).into(),
            stem: stem(&entry_path).to_owned(),
            ids,
            depth: todo.depth + 1,
        }))
    }
}
