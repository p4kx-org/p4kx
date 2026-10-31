// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod budget;
mod build;
mod lookup;

use p4kx_gltf::{GltfError, Scene, SceneNode};
use p4kx_model::{MaterialStatus, Model, Transform};

use crate::placement::{node_transform, offset};
use crate::{Assembly, Interior, Issue, IssueKind, ObjectContainerRef};
use lookup::near;

/// The name of the scene node that groups the Interior below the Entity.
pub const INTERIOR_NODE: &str = "Interior";

/// One container of an Entity's Interior: the reference, where it sits and what it holds.
#[derive(Debug, Clone)]
pub struct EntityContainer {
    pub(crate) part: usize,
    pub(crate) reference: ObjectContainerRef,
    pub(crate) placement: Transform,
    pub(crate) interior: Interior,
}

impl EntityContainer {
    /// Returns the index in [`Assembly::parts`] of the Entity that names the container: 0 for the
    /// root Entity, otherwise an attached one. The container sits relative to that part.
    #[must_use]
    pub const fn part(&self) -> usize {
        self.part
    }

    /// Returns the object container the Entity's Record names.
    #[must_use]
    pub const fn reference(&self) -> &ObjectContainerRef {
        &self.reference
    }

    /// Returns the transform from the container's space to that of its Entity, the part
    /// [`Self::part`]: the bone of the Entity's model moved by the Record's offset, or the bone
    /// alone when it already carries the offset.
    #[must_use]
    pub const fn placement(&self) -> &Transform {
        &self.placement
    }

    /// Returns the container's Interior.
    #[must_use]
    pub const fn interior(&self) -> &Interior {
        &self.interior
    }
}

/// The rooms of an Assembly: the object containers of the root Entity and of the attached
/// Entities, each placed relative to the Entity that names it.
///
/// Built by [`EntityInterior::build`] and put into an [`Assembly`] with
/// [`Assembly::with_interior`], which then writes it below an [`INTERIOR_NODE`] node of each
/// Entity that has containers.
#[derive(Debug, Clone, Default)]
pub struct EntityInterior {
    pub(crate) containers: Vec<EntityContainer>,
    pub(crate) issues: Vec<Issue>,
}

impl EntityInterior {
    /// Returns the containers that could be opened, in the order of the Record.
    #[must_use]
    pub fn containers(&self) -> &[EntityContainer] {
        &self.containers
    }

    /// Returns what could not be resolved, the Entity's containers first, then those of each
    /// container's Interior; the paths of the latter start with the container's name.
    #[must_use]
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    /// Returns the material libraries of the parts of all containers.
    pub(crate) fn libraries(&self) -> impl Iterator<Item = &MaterialStatus> {
        self.containers.iter().flat_map(|c| c.interior.libraries())
    }

    fn total(&self, count: fn(&Interior) -> usize) -> usize {
        self.containers.iter().map(|c| count(&c.interior)).sum()
    }

    /// Returns the number of parts of all containers.
    #[must_use]
    pub fn parts(&self) -> usize {
        self.total(|interior| interior.parts().len())
    }

    /// Returns the number of lights of all containers.
    #[must_use]
    pub fn lights(&self) -> usize {
        self.total(Interior::lights)
    }

    /// Returns the number of model parts of all containers, props included, readable or not.
    #[must_use]
    pub fn model_parts(&self) -> usize {
        self.total(Interior::model_parts)
    }

    /// Returns the number of parts of all containers that have a model.
    #[must_use]
    pub fn modelled_parts(&self) -> usize {
        self.total(Interior::modelled_parts)
    }

    fn place(
        &mut self,
        at: &str,
        reference: &ObjectContainerRef,
        model: Option<&Model>,
    ) -> Transform {
        let finite = reference
            .position()
            .iter()
            .chain(&reference.rotation())
            .all(|value| value.is_finite());
        let moved = if finite {
            offset(reference.position(), reference.rotation())
        } else {
            self.note(
                at,
                reference,
                IssueKind::InvalidOffset {
                    name: reference.file().to_owned(),
                },
            );
            Transform::IDENTITY
        };
        let bone = reference.bone();
        if bone.is_empty() {
            return moved;
        }
        let Some(anchor) = model.and_then(|model| node_transform(model, bone)) else {
            self.note(
                at,
                reference,
                IssueKind::HelperNotFound {
                    helper: bone.to_owned(),
                },
            );
            return moved;
        };
        let anchored = anchor.translation();
        if near(anchored, [0.0; 3]) {
            return anchor.then(&moved);
        }
        if finite && !near(reference.position(), [0.0; 3]) && !near(reference.position(), anchored)
        {
            self.note(
                at,
                reference,
                IssueKind::OffsetDiffers {
                    bone: bone.to_owned(),
                },
            );
        }
        anchor
    }

    fn add(
        &mut self,
        part: usize,
        owner_path: &str,
        reference: &ObjectContainerRef,
        placement: Transform,
        interior: Interior,
    ) {
        let name = interior.root().map_or("", |part| part.name());
        let prefix = if owner_path.is_empty() {
            name.to_owned()
        } else {
            format!("{owner_path}/{name}")
        };
        self.issues
            .extend(interior.issues().iter().map(|issue| Issue {
                path: if issue.path.is_empty() {
                    prefix.clone()
                } else {
                    format!("{prefix}/{}", issue.path)
                },
                name: issue.name.clone(),
                kind: issue.kind.clone(),
            }));
        self.containers.push(EntityContainer {
            part,
            reference: reference.clone(),
            placement,
            interior,
        });
    }

    fn note(&mut self, at: &str, reference: &ObjectContainerRef, kind: IssueKind) {
        self.issues.push(Issue {
            path: at.to_owned(),
            name: reference.file().to_owned(),
            kind,
        });
    }

    pub(crate) fn add_to_scene<'s>(
        &'s self,
        scene: &mut Scene<'s>,
        parts: usize,
    ) -> Result<(), GltfError> {
        let mut group: Option<(usize, usize)> = None;
        for container in &self.containers {
            let node = match group {
                Some((part, node)) if part == container.part => node,
                _ => {
                    let parent = (container.part < parts).then_some(container.part);
                    let node =
                        scene.push(SceneNode::new(INTERIOR_NODE, parent, Transform::IDENTITY))?;
                    group = Some((container.part, node));
                    node
                }
            };
            container
                .interior
                .add_to_scene(scene, Some(node), container.placement)?;
        }
        Ok(())
    }
}

impl Assembly {
    /// Returns the Interior of the Assembly, or `None` when none was added.
    #[must_use]
    pub const fn interior(&self) -> Option<&EntityInterior> {
        self.interior.as_ref()
    }

    /// Returns the Assembly with `interior` added: its parts are written below an
    /// [`INTERIOR_NODE`] node of each Entity that names containers (a root node when the Assembly
    /// has no parts), and its issues are appended to [`Self::issues`]. A second call replaces the
    /// first Interior and its issues.
    #[must_use]
    pub fn with_interior(mut self, interior: EntityInterior) -> Self {
        if let Some(before) = self.interior.take() {
            let kept = self.issues.len().saturating_sub(before.issues.len());
            self.issues.truncate(kept);
        }
        self.issues.extend(interior.issues.iter().cloned());
        self.interior = Some(interior);
        self
    }
}

#[cfg(test)]
mod tests;
