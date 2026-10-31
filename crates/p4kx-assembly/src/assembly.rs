// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::sync::Arc;

use p4kx_dataforge::Guid;
use p4kx_gltf::{
    GlbOptions, GlbSummary, GltfError, Scene, SceneNode, ScenePart, TextureLoader, write_scene_glb,
    write_scene_glb_with,
};
use p4kx_model::{MaterialStatus, Model, TintPalette, Transform, UnresolvedLayers};

use crate::{AttachedModel, Issue, creature_rig};

/// The most parts an Assembly holds; further parts of the Loadout are left out.
pub const MAX_ASSEMBLY_PARTS: usize = 1 << 14;

/// The most vertices the models of an Assembly hold, counted once per part; parts whose model
/// would pass it have no model.
pub const MAX_ASSEMBLY_VERTICES: usize = 1 << 25;

/// The deepest chain of parts below the root that is built.
pub const MAX_ASSEMBLY_DEPTH: usize = 2 * crate::MAX_LOADOUT_DEPTH + 2;

/// The choices of building an Assembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AssemblyOptions {
    /// The level of detail the models are read at, 0 for the most detailed; a model that lacks
    /// it is read at the nearest lower level it has.
    pub lod: u32,
    /// The most parts the Assembly holds, at most [`MAX_ASSEMBLY_PARTS`].
    pub max_parts: usize,
    /// The most vertices the models of the parts hold, at most [`MAX_ASSEMBLY_VERTICES`].
    pub max_vertices: usize,
}

impl Default for AssemblyOptions {
    fn default() -> Self {
        Self {
            lod: 0,
            max_parts: MAX_ASSEMBLY_PARTS,
            max_vertices: MAX_ASSEMBLY_VERTICES,
        }
    }
}

impl AssemblyOptions {
    /// Returns the options with the level of detail `lod`.
    #[must_use]
    pub const fn with_lod(mut self, lod: u32) -> Self {
        self.lod = lod;
        self
    }

    /// Returns the options with the limits `max_parts` and `max_vertices`, each lowered to
    /// [`MAX_ASSEMBLY_PARTS`] and [`MAX_ASSEMBLY_VERTICES`] when above.
    #[must_use]
    pub fn with_limits(mut self, max_parts: usize, max_vertices: usize) -> Self {
        self.max_parts = max_parts.min(MAX_ASSEMBLY_PARTS);
        self.max_vertices = max_vertices.min(MAX_ASSEMBLY_VERTICES);
        self
    }
}

/// How a part was put in its place.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Placement {
    /// The part is the root of the Assembly.
    Root,
    /// The part sits at the helper its port names in the parent's model: a node, else a bone of
    /// its skeleton in bind pose; below a skin part, else a bone of the root's skeleton.
    Helper(String),
    /// The part sits at the node of the parent's model named like its port.
    PortNode,
    /// The part is a skin: it is drawn in the parent's bind pose and follows the skeleton by its
    /// weights, so its port's helper and offset do not move it. A rigid prop on a skin port has no
    /// weights and hangs on a bone instead, see [`Placement::Helper`].
    Skin,
    /// The part sits at the parent's origin: its port names no helper or the model lacks it,
    /// or the parent has no model.
    Origin,
}

impl Placement {
    /// Returns the stable code of the placement, e.g. `helper`.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Helper(_) => "helper",
            Self::PortNode => "port_node",
            Self::Skin => "skin",
            Self::Origin => "origin",
        }
    }
}

/// One Entity placed in an [`Assembly`]: the root Entity or an Entity attached to a port of its
/// parent part.
#[derive(Debug, Clone)]
pub struct Part {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) guid: Guid,
    pub(crate) class_name: String,
    pub(crate) landing_gear: bool,
    pub(crate) parent: Option<usize>,
    pub(crate) local: Transform,
    pub(crate) world: Transform,
    pub(crate) placement: Placement,
    pub(crate) geometry: Option<String>,
    pub(crate) model: Option<Arc<Model>>,
    pub(crate) materials: Arc<MaterialStatus>,
    pub(crate) palette: Option<Arc<TintPalette>>,
    pub(crate) attachments: Vec<AttachedModel>,
    pub(crate) hidden: bool,
}

impl Part {
    /// Returns the part's name, `<port>/<entity>` with the port name and the Entity's class name,
    /// or the class name alone for the root.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the path of the part's port from the root, port names joined by `/`; empty for the
    /// root.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the GUID of the part's Entity; for a landing gear, that of the Entity it belongs
    /// to. Check [`Self::is_landing_gear`] before treating the part as an Entity.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the class name of the part's Entity; for a landing gear, the name of its bone.
    #[must_use]
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    /// Returns whether the part is a landing gear of the Entity it hangs below, not an Entity.
    #[must_use]
    pub const fn is_landing_gear(&self) -> bool {
        self.landing_gear
    }

    /// Returns the index of the parent part, or `None` for the root. A parent comes before its
    /// parts.
    #[must_use]
    pub const fn parent(&self) -> Option<usize> {
        self.parent
    }

    /// Returns the transform from the part's space to its parent's, in `CryEngine` coordinates,
    /// Z up and in metres: the port's helper in the parent's model, offset by the port.
    #[must_use]
    pub const fn local(&self) -> &Transform {
        &self.local
    }

    /// Returns the transform from the part's space to the Assembly's: the parents' transforms
    /// and this part's own.
    #[must_use]
    pub const fn world(&self) -> &Transform {
        &self.world
    }

    /// Returns how the part was put in its place.
    #[must_use]
    pub const fn placement(&self) -> &Placement {
        &self.placement
    }

    /// Returns the geometry path the part's Record names, or `None` when it names none.
    #[must_use]
    pub fn geometry(&self) -> Option<&str> {
        self.geometry.as_deref()
    }

    /// Returns the part's model, or `None` when the Entity has no geometry or it cannot be read.
    #[must_use]
    pub fn model(&self) -> Option<&Model> {
        self.model.as_deref()
    }

    /// Returns whether the part is hidden: it sits on a port flagged `invisible` or at a port a
    /// worn Entity hides by its `HiddenParts`, or below one, and has no model, as the game does
    /// not draw it.
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Returns the material library the part's model is drawn with: the one its Entity chooses,
    /// else the one the model names.
    #[must_use]
    pub fn materials(&self) -> &MaterialStatus {
        &self.materials
    }

    /// Returns the tint palette the part's materials that take one are drawn with: the one its
    /// Entity chooses, else the default of its material library; `None` when the Assembly was
    /// built without palettes or none could be resolved.
    #[must_use]
    pub fn palette(&self) -> Option<&TintPalette> {
        self.palette.as_deref()
    }

    /// Returns the rigid models the part carries at bones of its skeleton, the bone attachments
    /// of its character definition; empty for any other geometry.
    #[must_use]
    pub fn attachments(&self) -> &[AttachedModel] {
        &self.attachments
    }
}

/// An Entity with its Loadout as a scene graph of placed models: the root Entity's geometry and
/// every part of its Loadout at its port, each with its material library.
///
/// Built by [`Assembly::build`]. Models and material libraries are read once per file and shared
/// by the parts that use them.
#[derive(Debug, Clone)]
pub struct Assembly {
    pub(crate) parts: Vec<Part>,
    pub(crate) issues: Vec<Issue>,
    pub(crate) lod: u32,
    pub(crate) interior: Option<crate::EntityInterior>,
    pub(crate) containers: Vec<Arc<[crate::ObjectContainerRef]>>,
}

impl Assembly {
    /// Returns the parts, the root first, every part after its parent.
    #[must_use]
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// Returns the root part, the Entity the Assembly was built from.
    #[must_use]
    pub fn root(&self) -> Option<&Part> {
        self.parts.first()
    }

    /// Returns the part at the port `path`, port names joined by `/`, ignoring ASCII case.
    #[must_use]
    pub fn part_at(&self, path: &str) -> Option<&Part> {
        self.parts
            .iter()
            .find(|part| part.path.eq_ignore_ascii_case(path))
    }

    /// Returns what could not be resolved or done: first the attachments that name no part, then
    /// the issues of each part in the order of the parts.
    #[must_use]
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    /// Returns the layer files that the material libraries of the parts, their bone attachments
    /// and the Interior could not resolve; their materials keep the colours of their own file.
    #[must_use]
    pub fn unresolved_layers(&self) -> UnresolvedLayers {
        let parts = self.parts.iter().flat_map(|part| {
            std::iter::once(part.materials())
                .chain(part.attachments.iter().map(AttachedModel::materials))
        });
        let mut all = UnresolvedLayers::of(parts);
        if let Some(interior) = &self.interior {
            all.extend(interior.libraries());
        }
        all
    }

    /// Returns the level of detail the models were read at.
    #[must_use]
    pub const fn lod(&self) -> u32 {
        self.lod
    }

    /// Returns the bone attachments of all parts, each with the index of its part, in the order
    /// of the parts.
    pub fn attached_models(&self) -> impl Iterator<Item = (usize, &AttachedModel)> {
        self.parts.iter().enumerate().flat_map(|(index, part)| {
            part.attachments
                .iter()
                .map(move |attached| (index, attached))
        })
    }

    /// Returns the number of parts that have a model.
    #[must_use]
    pub fn modelled_parts(&self) -> usize {
        self.parts
            .iter()
            .filter(|part| part.model.is_some())
            .count()
    }

    /// Returns the number of hidden parts, see [`Part::is_hidden`].
    #[must_use]
    pub fn hidden_parts(&self) -> usize {
        self.parts.iter().filter(|part| part.hidden).count()
    }

    /// Writes the Assembly as a GLB file to `out`: a node per part named `<port>/<entity>` below
    /// the node of its parent part, placed by its transform, with the nodes of its model below it,
    /// and a node per bone attachment of a part below the part's node.
    /// The skins of a creature Entity, whose root geometry is a character file below
    /// `Objects/Characters` with a skeleton, are written bound to that skeleton, with its bones as
    /// nodes. `out` should be buffered. `options.lod` should be the level the Assembly was built
    /// at. The materials are plain colours, drawn with the palette of each part; see
    /// [`Self::write_glb_with`] for textures.
    ///
    /// # Errors
    ///
    /// Returns [`GltfError::TooLarge`] when the Assembly does not fit in a GLB file and
    /// [`GltfError::Write`] when `out` fails.
    pub fn write_glb(
        &self,
        options: &GlbOptions,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        write_scene_glb(&self.scene()?, options, out)
    }

    /// Writes the Assembly as a GLB file to `out` like [`Self::write_glb`], taking the textures
    /// of [`MaterialMode::Textures`](p4kx_gltf::MaterialMode::Textures) from `textures`.
    ///
    /// The mip level of `options` applies to every texture. An image several parts use is
    /// embedded once; the images of a part with a palette are shared with the parts of other
    /// palettes unless a specular map bakes the colours into the image. A texture that cannot be
    /// loaded leaves the material's plain colour and is listed in
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
        write_scene_glb_with(&self.scene()?, options, textures, out)
    }

    pub(crate) fn scene(&self) -> Result<Scene<'_>, GltfError> {
        let mut scene = creature_rig::skeleton_of(&self.parts, &self.issues, self.lod)
            .map_or_else(Scene::new, |skeleton| Scene::new().with_skeleton(skeleton));
        for part in &self.parts {
            let mut node = SceneNode::new(part.name.clone(), part.parent, part.local);
            if let Some(model) = &part.model {
                let drawn = ScenePart::new(model, &part.materials);
                node = node.with_part(
                    part.palette
                        .as_deref()
                        .map_or(drawn, |palette| drawn.with_palette(palette)),
                );
            }
            scene.push(node)?;
        }
        for (index, attached) in self.attached_models() {
            let drawn = ScenePart::new(&attached.model, &attached.materials);
            let palette = self
                .parts
                .get(index)
                .and_then(|part| part.palette.as_deref());
            let node = SceneNode::new(attached.name.clone(), Some(index), attached.local)
                .with_part(palette.map_or(drawn, |palette| drawn.with_palette(palette)));
            scene.push(node)?;
        }
        if let Some(interior) = &self.interior {
            interior.add_to_scene(&mut scene, self.parts.len())?;
        }
        Ok(scene)
    }
}
