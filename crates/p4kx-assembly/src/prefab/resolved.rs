// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_model::{PrefabObject, PrefabRef, Transform};

use super::{
    PrefabFile, PrefabGuidRemappingInstance, PrefabInstanceSource, PrefabIssue, PrefabLinkSource,
    PrefabLocation, PrefabObjectLocation, PrefabResolutionSource,
};

/// An object of a resolved prefab with its transform to the resolved prefab's space.
#[derive(Debug, Clone)]
pub struct ResolvedObject {
    pub(super) file: Arc<PrefabFile>,
    pub(super) prefab: usize,
    pub(super) object: usize,
    pub(super) world: Transform,
    pub(super) container: Arc<str>,
    pub(super) depth: usize,
    pub(super) route: Option<Arc<PrefabInstanceSource>>,
    pub(super) unexpanded_source: Option<Arc<PrefabLinkSource>>,
}

impl ResolvedObject {
    /// Returns the original file, Prefab row and flattened object ordinal.
    #[must_use]
    pub fn location(&self) -> PrefabObjectLocation<'_> {
        PrefabObjectLocation {
            prefab: PrefabLocation {
                file: &self.file,
                prefab_index: self.prefab,
            },
            object_index: self.object,
        }
    }

    /// Returns the innermost entered Prefab instance containing this object.
    #[must_use]
    pub fn instance_route(&self) -> Option<&PrefabInstanceSource> {
        self.route.as_deref()
    }

    /// Returns the evaluated target of a nested reference that stayed unexpanded.
    #[must_use]
    pub fn unexpanded_source(&self) -> Option<&PrefabLinkSource> {
        self.unexpanded_source.as_deref()
    }

    fn source(&self) -> Option<&PrefabObject> {
        self.file
            .library()
            .prefabs
            .get(self.prefab)
            .and_then(|prefab| prefab.objects.get(self.object))
    }

    /// Returns the object's `Name`.
    #[must_use]
    pub fn name(&self) -> &str {
        self.source().map_or("", |object| &object.name)
    }

    /// Returns the object's `Type`, e.g. `Entity`, `Brush` or `TagPoint`.
    #[must_use]
    pub fn kind(&self) -> &str {
        self.source().map_or("", |object| &object.kind)
    }

    /// Returns the object's `Id`.
    #[must_use]
    pub fn id(&self) -> &str {
        self.source().map_or("", |object| &object.id)
    }

    /// Returns the object's `EntityClass`, empty when it has none.
    #[must_use]
    pub fn class(&self) -> &str {
        self.source().map_or("", |object| &object.class)
    }

    /// Returns the geometry file the object names, a path below `Data`.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.source().and_then(|object| object.model.as_deref())
    }

    /// Returns the prefab the object instances when it was not expanded.
    #[must_use]
    pub fn unexpanded(&self) -> Option<&PrefabRef> {
        self.source().and_then(|object| object.nested.as_ref())
    }

    /// Returns whether the object is attached to another one, so that its transform is not
    /// relative to its prefab.
    #[must_use]
    pub fn attached(&self) -> bool {
        self.source().is_some_and(|object| object.attached)
    }

    /// Returns the transform from the object's space to its own prefab's, groups applied.
    #[must_use]
    pub fn local(&self) -> Transform {
        self.source()
            .map_or(Transform::IDENTITY, |object| object.transform)
    }

    /// Returns the transform from the object's space to the resolved prefab's: the transforms
    /// of the nested prefab objects around it composed with its own, rotation included, in
    /// `CryEngine` coordinates, Z up and in metres.
    #[must_use]
    pub const fn world(&self) -> &Transform {
        &self.world
    }

    /// Returns the names of the nested prefab objects the object sits in, joined by `/`; empty
    /// for an object of the resolved prefab itself.
    #[must_use]
    pub fn container(&self) -> &str {
        &self.container
    }

    /// Returns how many nested prefabs the object sits in.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the name of the prefab the object belongs to.
    #[must_use]
    pub fn prefab(&self) -> &str {
        self.file
            .library()
            .prefabs
            .get(self.prefab)
            .map_or("", |prefab| &prefab.name)
    }

    /// Returns the name of the library the object belongs to.
    #[must_use]
    pub fn library(&self) -> &str {
        &self.file.library().name
    }
}

/// A prefab with every nested prefab expanded.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ResolvedPrefab {
    /// The library of the prefab.
    pub library: String,
    /// The library file as `PrefabSet::library` and `resolve` take it: the path below
    /// `Data\Prefabs` without `.xml`.
    pub file: String,
    /// The prefab's name.
    pub name: String,
    /// The objects in file order, a nested prefab's objects in place of the object that
    /// instances it. An object that instances a prefab that was not expanded stays in the list.
    pub objects: Vec<ResolvedObject>,
    /// How many prefabs were entered, the resolved one included and nested ones counted every
    /// time they are instanced.
    pub prefabs: usize,
    /// What could not be resolved.
    pub issues: Vec<PrefabIssue>,
    pub(super) source: Option<PrefabResolutionSource>,
    pub(super) guid_remapping_instances: Vec<PrefabGuidRemappingInstance>,
}

impl ResolvedPrefab {
    /// Returns the distinct occurrences that author GUID remapping blocks.
    #[must_use]
    pub fn guid_remapping_instances(&self) -> &[PrefabGuidRemappingInstance] {
        &self.guid_remapping_instances
    }

    /// Returns original resolution evidence when the set was created with object sources.
    #[must_use]
    pub const fn source(&self) -> Option<&PrefabResolutionSource> {
        self.source.as_ref()
    }
}
