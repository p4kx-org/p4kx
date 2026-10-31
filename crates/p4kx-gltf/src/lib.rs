// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writes a [`p4kx_model::Model`] as one glTF 2.0 binary file (GLB).
//!
//! [`write_glb`] converts the model from `CryEngine` coordinates (Z up) to glTF's (Y up), keeps
//! metres, turns every node into a glTF node with its transform and every mesh into a glTF mesh
//! with one primitive per submesh. The model's material library becomes glTF materials: with
//! [`MaterialMode::Colours`] plain colours, opacity, roughness, metalness and emissive colour
//! without textures; submeshes whose material draws nothing are left out, and a material ID the
//! library lacks, or a model without a readable library, gets the engine's default material. With
//! [`MaterialMode::Textures`] and [`write_glb_with`] the diffuse, normal and specular maps of the
//! materials, which a [`TextureLoader`] supplies, are embedded as PNG images; one that cannot be
//! loaded leaves the plain colour and is reported. A skinned model is written with its skeleton:
//! the bones as nodes, one glTF skin over all of them with inverse bind matrices, and `JOINTS_n`
//! and `WEIGHTS_n` on the skinned meshes; a model with bones and no meshes is written as its bone
//! nodes. [`write_glb_animated`] adds glTF animations that move those bone nodes, or the nodes a
//! static model's helper bones drive, with rotation and translation channels. The binary data is
//! streamed to the writer after the JSON.
//!
//! [`write_scene_glb`] writes a [`Scene`]: a tree of named, placed nodes that hold models, each
//! drawn with the material library the scene gives it and, if the part has one, a
//! [`p4kx_model::TintPalette`], in plain colours and without skeletons unless
//! [`Scene::with_skeleton`] binds its skinned meshes to one. Equal materials are written once.
//! [`write_scene_glb_with`] adds the textures of [`MaterialMode::Textures`], shared between parts
//! like the materials. A scene node may also hold a [`SceneLight`], written with
//! the `KHR_lights_punctual` extension.
//!
//! [`write_rigged_glb`] writes [`RigPiece`]s worn on one shared skeleton, such as the pieces of
//! armour on a body: the bones once, one skin, and the bones of each piece matched to the
//! skeleton by name. [`write_rigged_glb_with`] adds the textures of the materials.

mod animations;
mod axes;
mod block_data;
mod document;
mod error;
mod glb;
mod glb_plan;
mod instance;
mod layer_bake;
mod layer_jobs;
mod layer_make;
mod layer_plan;
mod layer_view;
mod layout;
mod material_alpha;
mod material_index;
mod materials;
mod morphs;
mod nodes;
mod package_support;
mod rig;
mod rig_animation;
mod rig_mount;
mod scene;
mod scene_light;
mod scene_nodes;
mod scene_rig;
mod shading;
mod skin;
mod texture_assemble;
mod texture_budget;
mod texture_cache;
mod texture_loader;
mod texture_make;
mod texture_pixels;
mod texture_plan;
mod texture_report;
mod texture_roles;

pub use animations::AnimationSummary;
pub use error::GltfError;
pub use glb::{
    GlbOptions, GlbSummary, MaterialMode, write_glb, write_glb_animated, write_glb_animated_with,
    write_glb_with,
};
pub use layer_plan::draws_layer_diffuse;
pub use layer_view::{layered_base_key, layered_base_texture, layered_base_textures};
pub use materials::base_colour_factor;
pub use morphs::MorphTarget;
pub use package_support::{
    DrawnMaterial, NodeTransform, TextureFile, drawn_materials, light_node_transform,
    node_transform, texture_path,
};
pub use rig::{
    RigPiece, write_rigged_glb, write_rigged_glb_animated, write_rigged_glb_animated_with,
    write_rigged_glb_with,
};
pub use rig_animation::{RigAnimation, SampledAnimation, SampledBone, SampledWeights};
pub use rig_mount::RigMount;
pub use scene::{Scene, SceneNode, ScenePart, write_scene_glb, write_scene_glb_with};
pub use scene_light::{SceneLight, SceneLightKind};
pub use shading::{Shading, shading};
pub use texture_loader::{LoadedTexture, TextureLoadError, TextureLoader};
pub use texture_plan::{MAX_TEXTURE_IMAGES, MAX_TEXTURE_PIXELS};
pub use texture_report::{TextureFallback, TextureRole};
pub use texture_roles::diffuse_texture_path;
