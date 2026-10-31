// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io::Write;

use p4kx_model::{Animation, Bone, MaterialStatus, Model, TintPalette, Transform};

use crate::GltfError;
use crate::glb::{GlbOptions, GlbSummary, MaterialMode, write_instances};
use crate::instance::{Content, Instance, Wrapper};
use crate::materials::MaterialPlan;
use crate::scene_light::{LIGHT_FRAME, SceneLight};
use crate::scene_rig::SceneRig;
use crate::texture_loader::{NoTextures, TextureLoader};

type SharingKey = (usize, usize, usize, usize);

/// A model placed in a [`Scene`] with the material library its material IDs index.
///
/// The library comes from the caller, not from [`Model::materials`], so that a scene can draw a
/// model with the library its Entity chooses and share one library among many models.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct ScenePart<'m> {
    /// The model; its nodes hang below the scene node that holds the part.
    pub model: &'m Model,
    /// The material library of the model, see [`MaterialStatus::library`]; a status without a
    /// library draws the model with the engine's default material.
    pub materials: &'m MaterialStatus,
    /// The tint palette whose colours the materials that take one are drawn with, or `None` for
    /// the colours of the library.
    pub palette: Option<&'m TintPalette>,
}

impl<'m> ScenePart<'m> {
    /// Places `model`, drawn with the library of `materials` and without a palette.
    #[must_use]
    pub const fn new(model: &'m Model, materials: &'m MaterialStatus) -> Self {
        Self {
            model,
            materials,
            palette: None,
        }
    }

    /// Draws the part with the colours of `palette`.
    ///
    /// Parts share a mesh and its materials only when they also have the same palette, the same
    /// reference, so a livery never leaks into another part.
    #[must_use]
    pub const fn with_palette(mut self, palette: &'m TintPalette) -> Self {
        self.palette = Some(palette);
        self
    }
}

/// One node of a [`Scene`]: a name, a transform relative to its parent and optionally a model.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SceneNode<'m> {
    /// The node's name, e.g. `hardpoint_weapon_nose/AEGS_Mount_Gimbal_S3`.
    pub name: String,
    /// The index of the parent node, which comes before this one, or `None` for a root.
    pub parent: Option<usize>,
    /// The transform from the node's space to its parent's, in `CryEngine` coordinates.
    pub transform: Transform,
    /// The model placed at the node, if it has one.
    pub part: Option<ScenePart<'m>>,
    /// The light placed at the node, if it has one.
    pub light: Option<SceneLight>,
}

impl<'m> SceneNode<'m> {
    /// Creates a node named `name` below the node `parent`.
    #[must_use]
    pub fn new(name: impl Into<String>, parent: Option<usize>, transform: Transform) -> Self {
        Self {
            name: name.into(),
            parent,
            transform,
            part: None,
            light: None,
        }
    }

    /// Places `light` at the node; it is written with `KHR_lights_punctual`.
    ///
    /// A node with only a light is turned to glTF's light axis itself; a node that also has a part
    /// or children keeps its own frame, and the light sits on an extra child node named `<name> light`.
    #[must_use]
    pub const fn with_light(mut self, light: SceneLight) -> Self {
        self.light = Some(light);
        self
    }

    /// Places `part` at the node.
    #[must_use]
    pub const fn with_part(mut self, part: ScenePart<'m>) -> Self {
        self.part = Some(part);
        self
    }
}

/// A tree of placed models in `CryEngine` coordinates, Z up and in metres, for
/// [`write_scene_glb`](crate::write_scene_glb).
///
/// Every node comes after its parent, so a scene is a forest by construction.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Scene<'m> {
    nodes: Vec<SceneNode<'m>>,
    skeleton: Option<&'m [Bone]>,
    animations: Vec<&'m Animation>,
}

impl<'m> Scene<'m> {
    /// Creates a scene without nodes.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            nodes: Vec::new(),
            skeleton: None,
            animations: Vec::new(),
        }
    }

    /// Writes `animations` as glTF animations of the bones of the scene's skeleton.
    ///
    /// Each track moves the bone node with its controller ID; rotation and translation keys become
    /// one channel each, as in [`write_glb_animated`](crate::write_glb_animated). Tracks without
    /// a bone are left out and listed in [`GlbSummary::animations`]. Nothing is written when the
    /// scene has no skeleton or no skinned mesh, because then no bone nodes exist.
    #[must_use]
    pub fn with_animations(mut self, animations: impl IntoIterator<Item = &'m Animation>) -> Self {
        self.animations.extend(animations);
        self
    }

    /// Binds the skinned meshes of the scene's models to `skeleton`, by bone name.
    ///
    /// The writer then adds the bones as nodes and one glTF skin over them, and writes
    /// `JOINTS_n` and `WEIGHTS_n` on every skinned mesh, with its bones turned into the
    /// skeleton's; a bone the skeleton lacks takes its nearest ancestor that it has. The
    /// vertices of a skin are in the skeleton's space, so the node of a skinned mesh is a root
    /// without transform: the transforms of the scene nodes above it do not apply to it. The
    /// bones are written as roots in the scene's space, so the caller keeps the skins and the
    /// skeleton's own model at the identity. Nothing is added when no model has a skinned mesh.
    #[must_use]
    pub const fn with_skeleton(mut self, skeleton: &'m [Bone]) -> Self {
        self.skeleton = Some(skeleton);
        self
    }

    /// Returns the skeleton the skinned meshes are bound to, if the scene has one.
    pub(crate) const fn skeleton(&self) -> Option<&'m [Bone]> {
        self.skeleton
    }

    /// Adds `node` and returns its index.
    ///
    /// # Errors
    ///
    /// Returns [`GltfError::InvalidSceneNode`] when the node's parent is not an earlier node.
    pub fn push(&mut self, node: SceneNode<'m>) -> Result<usize, GltfError> {
        let index = self.nodes.len();
        if let Some(parent) = node.parent
            && parent >= index
        {
            return Err(GltfError::InvalidSceneNode {
                index,
                name: node.name,
                parent,
            });
        }
        self.nodes.push(node);
        Ok(index)
    }

    /// Returns the nodes in the order they were added.
    #[must_use]
    pub fn nodes(&self) -> &[SceneNode<'m>] {
        &self.nodes
    }
}

/// Writes the models of `scene` as one GLB file to `out`, like [`write_glb`]; `out` should be
/// buffered.
///
/// Every scene node becomes a glTF node with its transform, and the nodes of its model hang below
/// it. The materials are plain colours whatever [`GlbOptions::materials`] says, and the models are
/// written without skeletons unless the scene has one, see [`Scene::with_skeleton`]; see
/// [`write_scene_glb_with`] for textures.
///
/// Each model is drawn at the level of detail `options` asks for, or, when it lacks that level,
/// at the nearest lower one, or the lowest it has; [`GlbSummary::lod_fallbacks`] counts those.
/// Equal materials are written once, and so are the meshes of nodes that hold the same model
/// (the same reference) at the same level with the same material library and the same palette:
/// their glTF nodes reference one glTF mesh, counted in [`GlbSummary::shared_meshes`].
///
/// # Errors
///
/// Returns [`GltfError::TooLarge`] when the scene does not fit in a GLB file and
/// [`GltfError::Write`] when `out` fails.
pub fn write_scene_glb(
    scene: &Scene<'_>,
    options: &GlbOptions,
    out: &mut impl Write,
) -> Result<GlbSummary, GltfError> {
    let plain = GlbOptions {
        materials: MaterialMode::Colours,
        ..*options
    };
    write_scene_glb_with(scene, &plain, &NoTextures, out)
}

/// Writes the models of `scene` as one GLB file to `out` like [`write_scene_glb`], taking the
/// textures of [`MaterialMode::Textures`] from `textures`.
///
/// The textures of all parts are decoded, combined and PNG-encoded before the first byte is
/// written, a few at a time, at the mip level [`GlbOptions::texture_mip`] names. An image that
/// several materials or parts use is made and embedded once; a part with a [`TintPalette`] is
/// drawn with its colours, as the base colour factor over the image, which stays shared unless a
/// specular map bakes the colours into it (then each palette has its own). At most
/// [`crate::MAX_TEXTURE_IMAGES`] images and [`crate::MAX_TEXTURE_PIXELS`] pixels are embedded;
/// a texture that is beyond them or cannot be loaded leaves the plain colour and is listed in
/// [`GlbSummary::texture_fallbacks`].
///
/// # Errors
///
/// As [`write_scene_glb`].
pub fn write_scene_glb_with(
    scene: &Scene<'_>,
    options: &GlbOptions,
    textures: &dyn TextureLoader,
    out: &mut impl Write,
) -> Result<GlbSummary, GltfError> {
    let nodes = scene.nodes();
    let splits = light_splits(nodes);
    let light_names: Vec<String> = nodes
        .iter()
        .zip(&splits)
        .filter(|&(_, &split)| split)
        .map(|(node, _)| format!("{} light", node.name))
        .collect();
    let placed: Vec<Transform> = nodes
        .iter()
        .zip(&splits)
        .map(|(node, &split)| {
            if node.light.is_some() && !split {
                node.transform.then(&LIGHT_FRAME)
            } else {
                node.transform
            }
        })
        .collect();
    let mut wrappers: Vec<Wrapper<'_>> = nodes
        .iter()
        .zip(&placed)
        .zip(&splits)
        .map(|((node, transform), &split)| Wrapper {
            name: &node.name,
            parent: node.parent,
            transform,
            light: node.light.as_ref().filter(|_| !split),
        })
        .collect();
    wrappers.extend(
        nodes
            .iter()
            .enumerate()
            .zip(&splits)
            .filter(|&(_, &split)| split)
            .zip(&light_names)
            .map(|(((position, node), _), name)| Wrapper {
                name,
                parent: Some(position),
                transform: &LIGHT_FRAME,
                light: node.light.as_ref(),
            }),
    );
    let (instances, fallbacks) = instances_of(scene, options);
    let rig = scene
        .skeleton()
        .and_then(|skeleton| SceneRig::new(skeleton, &instances));
    let content = Content {
        wrappers: &wrappers,
        instances: &instances,
        rig: rig.as_ref(),
        animations: &scene.animations,
    };
    write_instances(&content, options, textures, fallbacks, out)
}

/// Returns for each node whether it holds a light and also a model or children, so that the
/// light's frame goes on a child node of its own.
fn light_splits(nodes: &[SceneNode<'_>]) -> Vec<bool> {
    let mut has_children = vec![false; nodes.len()];
    for node in nodes {
        if let Some(slot) = node.parent.and_then(|parent| has_children.get_mut(parent)) {
            *slot = true;
        }
    }
    nodes
        .iter()
        .zip(&has_children)
        .map(|(node, &children)| node.light.is_some() && (children || node.part.is_some()))
        .collect()
}

/// Returns the model instances of the nodes that hold a model, and how many are drawn at another
/// level than asked for. An instance whose model, level, library and palette an earlier one has
/// is its twin.
fn instances_of<'m>(scene: &Scene<'m>, options: &GlbOptions) -> (Vec<Instance<'m>>, usize) {
    let mut fallbacks = 0;
    let mut first_of: HashMap<SharingKey, usize> = HashMap::new();
    let mut instances: Vec<Instance<'m>> = Vec::new();
    for (position, node) in scene.nodes().iter().enumerate() {
        let Some(part) = node.part else {
            continue;
        };
        let lod = part.model.nearest_lod(options.lod);
        if lod.is_some_and(|lod| lod.level != options.lod) {
            fallbacks += 1;
        }
        let twin = lod.and_then(|lod| {
            let key = (
                std::ptr::from_ref(part.model).addr(),
                std::ptr::from_ref(lod).addr(),
                std::ptr::from_ref(part.materials).addr(),
                part.palette
                    .map_or(0, |palette| std::ptr::from_ref(palette).addr()),
            );
            match first_of.entry(key) {
                Entry::Occupied(first) => Some(*first.get()),
                Entry::Vacant(slot) => {
                    slot.insert(instances.len());
                    None
                }
            }
        });
        let plan = lod.map_or_else(MaterialPlan::empty, |lod| {
            let plan = MaterialPlan::new(part.materials.library(), lod);
            if twin.is_some() {
                plan
            } else {
                plan.tinted_by(part.palette)
            }
        });
        instances.push(Instance {
            model: part.model,
            lod,
            wrapper: Some(position),
            plan,
            twin,
        });
    }
    (instances, fallbacks)
}
