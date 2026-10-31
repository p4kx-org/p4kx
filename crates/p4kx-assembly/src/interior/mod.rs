// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::sync::Arc;

use p4kx_gltf::{
    GlbOptions, GlbSummary, GltfError, Scene, SceneLight, SceneNode, ScenePart, TextureLoader,
    write_scene_glb, write_scene_glb_with,
};
use p4kx_model::{Lod, MaterialStatus, Model, SocLight, TintPalette, Transform};

use crate::Issue;

mod build;
mod classes;
mod files;
mod layers;
mod light;
mod models;
mod scan;
mod unresolved;
mod walk;
mod watch;

pub use classes::InteriorForge;
pub use watch::{InteriorProgress, InteriorWatch};

pub(crate) fn model_vertices(model: &Model) -> usize {
    model.lods.iter().map(Lod::vertex_count).max().unwrap_or(0)
}

/// The most parts an Interior holds; further parts are left out.
pub const MAX_INTERIOR_PARTS: usize = 1 << 16;

/// The deepest chain of nested containers that is entered.
pub const MAX_INTERIOR_DEPTH: usize = 16;

/// The most containers one Interior opens, counting every nested one.
pub const MAX_INTERIOR_CONTAINERS: usize = 1 << 10;

/// The largest description or `.soc` file that is read, 128 MiB.
pub const MAX_INTERIOR_FILE_LEN: u64 = 128 << 20;

/// The choices of building an [`Interior`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct InteriorOptions {
    /// The level of detail the models are read at, 0 for the most detailed; a model that lacks
    /// it is read at the nearest lower level it has.
    pub lod: u32,
    /// The most parts the Interior holds, at most [`MAX_INTERIOR_PARTS`].
    pub max_parts: usize,
    /// The most vertices the distinct models of the parts hold together, at most
    /// [`MAX_ASSEMBLY_VERTICES`](crate::MAX_ASSEMBLY_VERTICES).
    pub max_vertices: usize,
    /// The most containers the Interior opens, the root and every nested one counted, at most
    /// [`MAX_INTERIOR_CONTAINERS`].
    pub max_containers: usize,
}

impl Default for InteriorOptions {
    fn default() -> Self {
        Self {
            lod: 0,
            max_parts: MAX_INTERIOR_PARTS,
            max_vertices: crate::MAX_ASSEMBLY_VERTICES,
            max_containers: MAX_INTERIOR_CONTAINERS,
        }
    }
}

impl InteriorOptions {
    /// Returns the options with the level of detail `lod`.
    #[must_use]
    pub const fn with_lod(mut self, lod: u32) -> Self {
        self.lod = lod;
        self
    }

    /// Returns the options with at most `max_containers` containers, lowered to
    /// [`MAX_INTERIOR_CONTAINERS`] when above.
    #[must_use]
    pub fn with_max_containers(mut self, max_containers: usize) -> Self {
        self.max_containers = max_containers.min(MAX_INTERIOR_CONTAINERS);
        self
    }

    /// Returns the options with the limits `max_parts` and `max_vertices`, each lowered to its
    /// maximum when above.
    #[must_use]
    pub fn with_limits(mut self, max_parts: usize, max_vertices: usize) -> Self {
        self.max_parts = max_parts.min(MAX_INTERIOR_PARTS);
        self.max_vertices = max_vertices.min(crate::MAX_ASSEMBLY_VERTICES);
        self
    }
}

/// What a part of an [`Interior`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InteriorPartKind {
    /// A container: the root, or one nested in another at a placement.
    Container,
    /// A `.soc` file a container streams; it shares its container's space.
    Stream,
    /// A model a `.soc` file places.
    Model,
    /// A prop: an entity of a `.soc` file that names geometry of its own, or whose class Record
    /// does.
    Prop,
    /// A light of a `.soc` file that glTF can express: point, spot or directional.
    Light,
}

impl InteriorPartKind {
    /// Returns the stable code of the kind, e.g. `model`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::Stream => "stream",
            Self::Model => "model",
            Self::Prop => "prop",
            Self::Light => "light",
        }
    }

    /// Returns whether a part of this kind is drawn with a model of its own.
    #[must_use]
    pub const fn has_model(self) -> bool {
        matches!(self, Self::Model | Self::Prop)
    }
}

/// One part of an [`Interior`]: a container, a `.soc` file it streams or a model placed in it.
#[derive(Debug, Clone)]
pub struct InteriorPart {
    pub(crate) name: String,
    pub(crate) kind: InteriorPartKind,
    pub(crate) parent: Option<usize>,
    pub(crate) local: Transform,
    pub(crate) world: Transform,
    pub(crate) model_path: Option<String>,
    pub(crate) model: Option<Arc<Model>>,
    pub(crate) materials: Arc<MaterialStatus>,
    pub(crate) palette: Option<Arc<TintPalette>>,
    pub(crate) light: Option<SocLight>,
}

impl InteriorPart {
    /// Returns the tint palette the part is drawn with: the one its `.soc` file or class Record
    /// chooses, else the default of its material library; `None` without an
    /// [`InteriorForge`] or when there is none.
    #[must_use]
    pub fn palette(&self) -> Option<&TintPalette> {
        self.palette.as_deref()
    }

    /// Returns the light of a [`Light`](InteriorPartKind::Light) part as the `.soc` file states
    /// it.
    #[must_use]
    pub const fn light(&self) -> Option<&SocLight> {
        self.light.as_ref()
    }

    /// Returns the light of a [`Light`](InteriorPartKind::Light) part as glTF expresses it: its
    /// colour linear, its intensity the engine's raw value, its range the light radius.
    #[must_use]
    pub fn scene_light(&self) -> Option<SceneLight> {
        self.light.as_ref().and_then(light::scene_light)
    }

    /// Returns the part's name: the container's name, the `.soc` file's name without extension or
    /// the model file's name without extension.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns what the part is.
    #[must_use]
    pub const fn kind(&self) -> InteriorPartKind {
        self.kind
    }

    /// Returns the index of the parent part, or `None` for the root. A parent comes before its
    /// parts.
    #[must_use]
    pub const fn parent(&self) -> Option<usize> {
        self.parent
    }

    /// Returns the transform from the part's space to its parent's, in `CryEngine` coordinates, Z
    /// up and in metres.
    #[must_use]
    pub const fn local(&self) -> &Transform {
        &self.local
    }

    /// Returns the transform from the part's space to the Interior's: the parents' transforms
    /// and this part's own.
    #[must_use]
    pub const fn world(&self) -> &Transform {
        &self.world
    }

    /// Returns the path of the part's model as the `.soc` file spells it, or `None` for a part
    /// that is no model.
    #[must_use]
    pub fn model_path(&self) -> Option<&str> {
        self.model_path.as_deref()
    }

    /// Returns the part's model, or `None` when the part is no model or it cannot be read.
    #[must_use]
    pub fn model(&self) -> Option<&Model> {
        self.model.as_deref()
    }

    /// Returns the material library the part's model is drawn with.
    #[must_use]
    pub fn materials(&self) -> &MaterialStatus {
        &self.materials
    }
}

/// The rooms of an Entity, or a container on its own, as a scene graph of placed models.
///
/// It holds the models, props and lights of a socpak container's `.soc` files and of the
/// containers nested in it, each at its transform composed with those of the containers around
/// it.
///
/// Built by [`Interior::build`]. Models and material libraries are read once per file and shared
/// by the parts that use them.
#[derive(Debug, Clone)]
pub struct Interior {
    pub(crate) parts: Vec<InteriorPart>,
    pub(crate) issues: Vec<Issue>,
    pub(crate) lod: u32,
    pub(crate) containers: usize,
}

impl Interior {
    /// Returns the parts, the root container first, every part after its parent.
    #[must_use]
    pub fn parts(&self) -> &[InteriorPart] {
        &self.parts
    }

    /// Returns the root part, the container the Interior was built from.
    #[must_use]
    pub fn root(&self) -> Option<&InteriorPart> {
        self.parts.first()
    }

    /// Returns what could not be resolved or done, in the order it was found.
    #[must_use]
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    /// Returns the level of detail the models were read at.
    #[must_use]
    pub const fn lod(&self) -> u32 {
        self.lod
    }

    /// Returns the number of containers that were opened, the root included.
    #[must_use]
    pub const fn containers(&self) -> usize {
        self.containers
    }

    /// Returns the vertices of the distinct models of the parts, as the vertex limit counts them.
    #[must_use]
    pub fn vertices(&self) -> usize {
        let mut seen: Vec<*const Model> = Vec::new();
        let mut total = 0_usize;
        for model in self.parts.iter().filter_map(|part| part.model.as_ref()) {
            let pointer = Arc::as_ptr(model);
            if !seen.contains(&pointer) {
                seen.push(pointer);
                total = total.saturating_add(model_vertices(model));
            }
        }
        total
    }

    /// Returns the number of parts with a model of their own, placed models and props, readable
    /// or not.
    #[must_use]
    pub fn model_parts(&self) -> usize {
        self.parts
            .iter()
            .filter(|part| part.kind.has_model())
            .count()
    }

    /// Returns the number of props, readable or not.
    #[must_use]
    pub fn props(&self) -> usize {
        self.count(InteriorPartKind::Prop)
    }

    /// Returns the number of lights.
    #[must_use]
    pub fn lights(&self) -> usize {
        self.count(InteriorPartKind::Light)
    }

    fn count(&self, kind: InteriorPartKind) -> usize {
        self.parts.iter().filter(|part| part.kind == kind).count()
    }

    /// Returns the number of parts that have a model.
    #[must_use]
    pub fn modelled_parts(&self) -> usize {
        self.parts
            .iter()
            .filter(|part| part.model.is_some())
            .count()
    }

    /// Adds the parts to `scene`, the root container below the node `parent` placed by
    /// `placement`, every other part below its own parent; returns the index of the root's node,
    /// or `None` when the Interior has no parts and nothing was added.
    ///
    /// # Errors
    ///
    /// Returns [`GltfError::InvalidSceneNode`] when `parent` is not a node of `scene`.
    pub fn add_to_scene<'s>(
        &'s self,
        scene: &mut Scene<'s>,
        parent: Option<usize>,
        placement: Transform,
    ) -> Result<Option<usize>, GltfError> {
        let mut nodes = Vec::with_capacity(self.parts.len());
        for part in &self.parts {
            let (above, local) = part.parent.map_or((parent, placement), |index| {
                (nodes.get(index).copied(), part.local)
            });
            let mut node = SceneNode::new(part.name.clone(), above, local);
            if let Some(model) = &part.model {
                let mut scene_part = ScenePart::new(model, &part.materials);
                if let Some(palette) = &part.palette {
                    scene_part = scene_part.with_palette(palette);
                }
                node = node.with_part(scene_part);
            }
            if let Some(light) = part.scene_light() {
                node = node.with_light(light);
            }
            nodes.push(scene.push(node)?);
        }
        Ok(nodes.first().copied())
    }

    /// Writes the Interior as a GLB file to `out`: a node per part below the node of its parent,
    /// placed by its transform, with the nodes of its model below it; `out` should be buffered.
    /// `options.lod` should be the level the Interior was built at.
    ///
    /// # Errors
    ///
    /// Returns [`GltfError::TooLarge`] when the Interior does not fit in a GLB file and
    /// [`GltfError::Write`] when `out` fails.
    pub fn write_glb(
        &self,
        options: &GlbOptions,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        let mut scene = Scene::new();
        self.add_to_scene(&mut scene, None, Transform::IDENTITY)?;
        write_scene_glb(&scene, options, out)
    }

    /// Writes the Interior as a GLB file to `out` like [`Self::write_glb`], taking the textures
    /// of [`MaterialMode::Textures`](p4kx_gltf::MaterialMode::Textures) from `textures`; a texture
    /// that cannot be loaded leaves the material's plain colour and is listed in
    /// [`GlbSummary::texture_fallbacks`].
    ///
    /// # Errors
    ///
    /// As [`Self::write_glb`].
    pub fn write_glb_with(
        &self,
        options: &GlbOptions,
        textures: &dyn TextureLoader,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        let mut scene = Scene::new();
        self.add_to_scene(&mut scene, None, Transform::IDENTITY)?;
        write_scene_glb_with(&scene, options, textures, out)
    }
}
