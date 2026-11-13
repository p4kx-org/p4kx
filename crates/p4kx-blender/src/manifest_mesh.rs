// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::manifest_rig::Rig;

/// One GLB mesh: the geometry of a model drawn with one material library and one palette.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Mesh {
    /// The mesh's id, e.g. `m0001`; also the stem of its file.
    pub id: String,
    /// The GLB file relative to the package folder, e.g. `meshes/m0001.glb`.
    pub file: String,
    /// The geometry file in the Archive the model was read from, below `Data`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<String>,
    /// The name of the tint palette the mesh's colours were taken from, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// The id of that palette in `palettes`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette_id: Option<String>,
    /// The number of nodes that draw this mesh; more than one means the mesh is instanced.
    pub instances: usize,
    /// The size of the file in bytes.
    pub bytes: u64,
    /// The number of vertices of the mesh.
    pub vertices: usize,
    /// The number of triangles of the mesh.
    pub triangles: usize,
    /// The materials the mesh is drawn with, in the order of the glTF materials in its file; empty
    /// for a mesh with a `rig`, whose materials are in the file.
    pub materials: Vec<MeshMaterial>,
    /// The skeleton, skinned parts, morph targets and animations of the file; present for a
    /// Character (version 1.3) and a creature with its skeleton (1.8), absent for a static mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rig: Option<Rig>,
}

/// A material of a mesh and the textures its material file names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MeshMaterial {
    /// The index of the glTF material in the mesh's file.
    pub index: usize,
    /// The name of the glTF material in the mesh's file.
    pub name: String,
    /// The textures that were written for the material.
    pub textures: MaterialTextures,
    /// The id of the material description, absent for a synthetic fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// Raw geometry material IDs assigned to this dense material index, in ascending order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_material_ids: Vec<u16>,
    /// Whether this assignment uses the physical source described by `default_material`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archive_default: bool,
    /// Authored library root examined for this assignment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authored_library: Option<usize>,
    /// Why each affected raw material ID needed a fallback.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallback_slots: Vec<crate::FallbackMaterialSlot>,
}

/// The texture ids of a material by role; a role is absent when the material names no texture for
/// it or the texture could not be written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MaterialTextures {
    /// The diffuse map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffuse: Option<String>,
    /// The normal map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal: Option<String>,
    /// The specular map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specular: Option<String>,
}
