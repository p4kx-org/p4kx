// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a skinned part of a rig comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RigPartOrigin {
    /// The skin of the default body.
    Body,
    /// The skin of a piece: the head, an item of the head or a piece of armour or clothing.
    Piece,
    /// A skin of a creature, bound to the skeleton of its root.
    Skin,
}

/// The skeleton, skins, morph targets and animations a mesh file carries.
///
/// A mesh with a `rig` is a skinned Character or creature: the GLB holds the bones as nodes, one
/// glTF skin over them, a skinned mesh per part, the morph targets of the parts and the
/// animations, and a reader imports it once, not as an instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Rig {
    /// The code of the body variant the Character is worn on, e.g. `male`; absent for a creature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// The file the bones come from, below `Data`: the skeleton of a Character, the geometry of
    /// the root of a creature.
    pub skeleton: String,
    /// The number of bones written as nodes.
    pub bones: usize,
    /// The number of meshes that carry bone weights.
    pub skinned_meshes: usize,
    /// The number of morph targets over all meshes.
    pub morph_targets: usize,
    /// The number of PNG images embedded in the GLB file; the textures of a rig are not written as
    /// files of the package.
    pub embedded_textures: usize,
    /// The skins worn on the skeleton, the body first.
    pub parts: Vec<RigPart>,
    /// The animations in the GLB file, in order.
    pub animations: Vec<RigClip>,
}

/// One skin worn on the skeleton of a Character or creature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RigPart {
    /// The name of the part's node in the GLB file.
    pub name: String,
    /// Where the part comes from.
    pub origin: RigPartOrigin,
    /// The skin file the part was read from, below `Data`.
    pub file: String,
    /// The number of morph targets of the part's mesh.
    pub morph_targets: usize,
}

/// One animation of a Character as the GLB file holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RigClip {
    /// The animation's name, also the name of the glTF animation.
    pub name: String,
    /// The length in seconds.
    pub duration_seconds: f32,
    /// Whether the keys are offsets to be added to a pose, which glTF cannot say.
    pub additive: bool,
    /// The number of channels: rotation, translation and morph weight tracks.
    pub channels: usize,
    /// The number of keys.
    pub keys: usize,
}
