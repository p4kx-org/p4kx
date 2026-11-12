// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! What `p4kx model info --json` and `p4kx chunks list --json` say, as serialisable shapes.

use std::borrow::Cow;

use p4kx_chunks::Chunk;
use p4kx_model::{
    Bone, BoneTransform, ChunkUsage, Mesh, Model, ModelChunk, ModelFile, Node, Submesh,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::model_info_animations::{AnimationInfo, UnreadInfo};
use crate::model_info_materials::{LibraryInfo, MaterialInfo};
use crate::{AnimationListing, AnimationSet, ModelSource};

/// What `p4kx model info --json` says about a model, and what front-ends serialise of it.
#[derive(Debug, Serialize)]
pub struct ModelInfo<'a> {
    /// The Entry path or file of the model.
    pub model: &'a str,
    /// The geometry companion the model names.
    pub geometry: Option<&'a str>,
    /// The material library the model names.
    pub material_library: Option<&'a str>,
    /// The material IDs the meshes use.
    pub material_ids: Vec<u16>,
    /// What reading the material library came to.
    pub library: LibraryInfo<'a>,
    /// The materials of the library.
    pub materials: Vec<MaterialInfo<'a>>,
    /// The list of material indices of the node table.
    pub node_material_indices: &'a [u16],
    /// What a lenient read worked around, as sentences.
    pub issues: Vec<String>,
    /// The nodes of the model.
    pub nodes: Vec<NodeInfo<'a>>,
    /// The levels of detail with their meshes.
    pub lods: Vec<LodInfo>,
    /// The bones of the skeleton.
    pub bones: Vec<BoneInfo<'a>>,
    /// The animations read for the model.
    pub animations: Vec<AnimationInfo>,
    /// The animation files and animations that could not be read.
    pub unread_animations: Vec<UnreadInfo>,
    /// Every chunk of the model's files.
    pub chunks: Vec<ChunkInfo>,
}

impl<'a> ModelInfo<'a> {
    /// Describes `source` and the `animations` read for it.
    #[must_use]
    pub fn of(source: &'a ModelSource, animations: &AnimationSet) -> Self {
        Self::with_animations(
            source,
            AnimationInfo::of_set(source.model(), animations),
            animations.unread.iter().map(UnreadInfo::of).collect(),
        )
    }

    /// Describes `source` and the `listing` of the animations found for it.
    #[must_use]
    pub fn of_listing(source: &'a ModelSource, listing: &AnimationListing) -> Self {
        Self::with_animations(
            source,
            AnimationInfo::of_listing(source.model(), listing),
            listing.unread.iter().map(UnreadInfo::of).collect(),
        )
    }

    fn with_animations(
        source: &'a ModelSource,
        animations: Vec<AnimationInfo>,
        unread_animations: Vec<UnreadInfo>,
    ) -> Self {
        let model = source.model();
        Self {
            model: source.name(),
            geometry: model.geometry.as_deref(),
            material_library: model.material_library.as_deref(),
            material_ids: model.material_ids(),
            library: LibraryInfo::of(model),
            materials: MaterialInfo::of_model(model),
            node_material_indices: &model.material_indices,
            issues: model.issues.iter().map(ToString::to_string).collect(),
            nodes: NodeInfo::of_model(model),
            lods: LodInfo::of_model(model),
            bones: BoneInfo::of_model(model),
            animations,
            unread_animations,
            chunks: ChunkInfo::of_model(model),
        }
    }
}

/// One node of a model.
#[derive(Debug, Serialize)]
pub struct NodeInfo<'a> {
    index: usize,
    name: &'a str,
    parent: Option<usize>,
    transform: [[f32; 4]; 3],
    scale: [f32; 3],
    geometry_type: Option<u16>,
    helper: bool,
    properties: Vec<PropertyInfo<'a>>,
}

#[derive(Debug, Serialize)]
struct PropertyInfo<'a> {
    key: &'a str,
    value: &'a str,
}

impl<'a> NodeInfo<'a> {
    /// Describes the nodes of `model`.
    #[must_use]
    pub fn of_model(model: &'a Model) -> Vec<Self> {
        model.nodes.iter().enumerate().map(Self::of).collect()
    }

    fn of((index, node): (usize, &'a Node)) -> Self {
        Self {
            index,
            name: &node.name,
            parent: node.parent,
            transform: node.transform.rows,
            scale: node.scale,
            geometry_type: node.geometry_type,
            helper: node.is_helper(),
            properties: node
                .properties
                .iter()
                .map(|(key, value)| PropertyInfo { key, value })
                .collect(),
        }
    }
}

/// One level of detail of a model.
#[derive(Debug, Serialize)]
pub struct LodInfo {
    /// The level of detail, 0 for the finest.
    pub level: u32,
    /// The vertices of all meshes of the level.
    pub vertices: usize,
    /// The triangles of all meshes of the level.
    pub triangles: usize,
    /// The number of bones of the skeleton the level carries of its own, when it differs from the
    /// model's; `None` for a level that uses the model's skeleton.
    pub skeleton_bones: Option<usize>,
    /// The meshes of the level.
    pub meshes: Vec<MeshInfo>,
}

impl LodInfo {
    /// Describes the levels of detail of `model` with their meshes.
    #[must_use]
    pub fn of_model(model: &Model) -> Vec<Self> {
        model
            .lods
            .iter()
            .map(|lod| Self {
                level: lod.level,
                vertices: lod.vertex_count(),
                triangles: lod.triangle_count(),
                skeleton_bones: lod.bones.as_ref().map(Vec::len),
                meshes: lod.meshes.iter().map(MeshInfo::of).collect(),
            })
            .collect()
    }
}

/// One mesh of a level of detail.
#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the JSON has one flag per optional vertex stream of a mesh"
)]
pub struct MeshInfo {
    node: usize,
    vertices: usize,
    triangles: usize,
    normals: bool,
    normals_computed: bool,
    tangents: bool,
    colours: bool,
    second_uvs: bool,
    non_finite_uvs: usize,
    skinned: bool,
    submeshes: Vec<SubmeshInfo>,
}

impl MeshInfo {
    fn of(mesh: &Mesh) -> Self {
        Self {
            node: mesh.node,
            vertices: mesh.positions.len(),
            triangles: mesh.indices.len() / 3,
            normals: mesh.normals.is_some(),
            normals_computed: mesh.normals_computed,
            tangents: mesh.tangents.is_some(),
            colours: mesh.colours.is_some(),
            second_uvs: mesh.second_uvs.is_some(),
            non_finite_uvs: mesh.non_finite_uvs,
            skinned: mesh.influences.is_some(),
            submeshes: mesh.submeshes.iter().map(SubmeshInfo::of).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct SubmeshInfo {
    material_id: u16,
    first_index: usize,
    index_count: usize,
}

impl SubmeshInfo {
    const fn of(submesh: &Submesh) -> Self {
        Self {
            material_id: submesh.material_id,
            first_index: submesh.first_index,
            index_count: submesh.index_count,
        }
    }
}

/// One bone of a model's skeleton.
#[derive(Debug, Serialize)]
pub struct BoneInfo<'a> {
    index: usize,
    name: &'a str,
    parent: Option<usize>,
    controller_id: u32,
    node: Option<usize>,
    relative: PoseInfo,
    world: PoseInfo,
}

impl<'a> BoneInfo<'a> {
    /// Describes the bones of `model`.
    #[must_use]
    pub fn of_model(model: &'a Model) -> Vec<Self> {
        model
            .bones
            .iter()
            .enumerate()
            .map(|(index, bone)| Self::of_bone(index, bone))
            .collect()
    }

    /// Describes the selected original bone at its model ordinal.
    #[must_use]
    pub fn of_bone(index: usize, bone: &'a Bone) -> Self {
        Self {
            index,
            name: &bone.name,
            parent: bone.parent,
            controller_id: bone.controller_id,
            node: bone.node,
            relative: PoseInfo::of(&bone.relative),
            world: PoseInfo::of(&bone.world),
        }
    }
}

#[derive(Debug, Serialize)]
struct PoseInfo {
    rotation: [f32; 4],
    translation: [f32; 3],
}

impl PoseInfo {
    const fn of(pose: &BoneTransform) -> Self {
        Self {
            rotation: pose.rotation,
            translation: pose.translation,
        }
    }
}

/// One chunk of a model's files.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = crate::projection_schema::require_properties)]
pub struct ChunkInfo {
    file: Cow<'static, str>,
    #[schemars(range(max = u32::MAX))]
    index: usize,
    #[serde(rename = "type")]
    type_name: Option<&'static str>,
    #[schemars(range(max = u32::MAX))]
    type_id: u32,
    #[schemars(extend("enum" = ["read", "skipped", "unknown"]))]
    usage: &'static str,
}

impl ChunkInfo {
    /// Describes the chunks of the files of `model`.
    #[must_use]
    pub fn of_model(model: &Model) -> Vec<Self> {
        model.chunks.iter().map(Self::of).collect()
    }

    /// Describes `chunk`.
    #[must_use]
    pub fn of(chunk: &ModelChunk) -> Self {
        Self {
            file: file_name(chunk.file),
            index: chunk.index,
            type_name: chunk.chunk_type.name(),
            type_id: chunk.chunk_type.id(),
            usage: usage_name(chunk.usage),
        }
    }
}

/// One entry of the chunk table of a chunk file.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ChunkJson {
    /// The position in the chunk table, counted from 0.
    pub index: usize,
    /// The type name, absent for a type not in the table of known types.
    #[serde(rename = "type")]
    pub type_name: Option<&'static str>,
    /// The numeric type ID.
    pub type_id: u32,
    /// Whether the type is in the table of known types.
    pub known: bool,
    /// The chunk version.
    pub version: u32,
    /// The chunk ID, absent in the `#ivo` form.
    pub id: Option<u32>,
    /// The byte order of the chunk's data: `little` or `big`.
    pub byte_order: &'static str,
    /// The offset of the chunk's data in the file.
    pub offset: u64,
    /// The size of the chunk's data in bytes.
    pub size: u64,
}

impl ChunkJson {
    /// Describes `chunk`, the entry `index` of its file's chunk table.
    #[must_use]
    pub fn of(index: usize, chunk: &Chunk) -> Self {
        let chunk_type = chunk.chunk_type();
        Self {
            index,
            type_name: chunk_type.name(),
            type_id: chunk_type.id(),
            known: chunk_type.is_known(),
            version: chunk.version(),
            id: chunk.id(),
            byte_order: chunk.byte_order().name(),
            offset: chunk.offset(),
            size: chunk.size(),
        }
    }
}

/// Returns the logical model or geometry role, including a level-specific file.
#[must_use]
pub fn file_name(file: ModelFile) -> Cow<'static, str> {
    match file {
        ModelFile::Model => Cow::Borrowed("model"),
        ModelFile::Geometry => Cow::Borrowed("geometry"),
        ModelFile::LodGeometry(level) => Cow::Owned(format!("lod{level}-geometry")),
        ModelFile::LodModel(level) => Cow::Owned(format!("lod{level}-model")),
        _ => Cow::Borrowed("other"),
    }
}

/// Returns `read`, `skipped` or `unknown`.
#[must_use]
pub const fn usage_name(usage: ChunkUsage) -> &'static str {
    match usage {
        ChunkUsage::Read => "read",
        ChunkUsage::Skipped => "skipped",
        ChunkUsage::Unknown => "unknown",
    }
}

/// Returns the name of the node `index`, or `-` when it has none.
#[must_use]
pub fn node_name(model: &Model, index: usize) -> &str {
    model
        .nodes
        .get(index)
        .map(|node| node.name.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("-")
}
