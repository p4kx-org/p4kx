// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::TestChunkFile;
use crate::model_cube::{cube_nodes, cube_submeshes, cube_vertices};
pub use crate::model_layout::{TestNormals, TestPositions};
use crate::model_nmc::{NodeDetails, nmc_chunk};
use crate::model_skin::{skin_chunk, tangent_frame};

const EXPORT_FLAGS: u32 = 0xBE5E_493E;
const MTL_NAME_320: u32 = 0x8335_3333;
const NODE_MESH_COMBO: u32 = 0x7069_7FDA;
const MESH_INFO: u32 = 0x9291_4444;
const SKIN_2: u32 = 0xB875_7777;
const TERMINATOR: u32 = 0xE018_1074;
const IVO_VERSION: u32 = 0x900;

/// Whether the stream has eight influences per vertex, and the (bone, weight) pairs of each vertex.
pub(crate) type BoneMap = (bool, Vec<Vec<(u16, u8)>>);

/// One node of a [`TestModel`]'s `NodeMeshCombo` chunk.
#[derive(Debug, Clone, PartialEq)]
pub struct TestNode {
    /// The node's name.
    pub name: String,
    /// The index of the parent node, or `None` for a root.
    pub parent: Option<u16>,
    /// The transform relative to the parent, 3×4 row-major with the translation last.
    pub local: [[f32; 4]; 3],
    /// The node type: 0 for geometry, 2 or 3 for helpers.
    pub kind: u16,
}

/// One vertex of a [`TestModel`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TestVertex {
    /// The position in the node's space, in metres, Z up.
    pub position: [f32; 3],
    /// The unit normal.
    pub normal: [f32; 3],
    /// The texture coordinate.
    pub uv: [f32; 2],
}

/// One submesh of a [`TestModel`]: a range of indices drawn with one material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestSubmesh {
    /// The material ID.
    pub material_id: u16,
    /// The index of the node the submesh belongs to.
    pub node: u16,
    /// The first index of the range.
    pub first_index: u32,
    /// The number of indices.
    pub index_count: u32,
    /// The first vertex the range uses.
    pub first_vertex: u32,
    /// The number of vertices the range uses.
    pub vertex_count: u32,
}

/// A synthetic static `#ivo` model: the `.cgf` with its nodes and material library and the
/// `.cgfm` geometry companion with its mesh.
///
/// [`Self::cube`] is a cube of 4 m edge length standing on the ground, Z up: its vertices span
/// −2…2 in X and Y and 0…4 in Z. The root node `cube` holds the geometry node `cube_mesh`,
/// translated by [`CUBE_MESH_OFFSET`]. Its top and bottom faces use material ID
/// [`CUBE_CAPS_MATERIAL`], its four sides [`CUBE_SIDES_MATERIAL`].
#[derive(Debug, Clone, PartialEq)]
pub struct TestModel {
    pub(crate) nodes: Vec<TestNode>,
    pub(crate) vertices: Vec<TestVertex>,
    pub(crate) indices: Vec<u32>,
    pub(crate) submeshes: Vec<TestSubmesh>,
    pub(crate) material_library: String,
    pub(crate) positions: TestPositions,
    pub(crate) normals: TestNormals,
    pub(crate) wide_indices: bool,
    pub(crate) model_box: Option<([f32; 3], [f32; 3])>,
    pub(crate) repeated_index_streams: usize,
    pub(crate) colours: Option<Vec<[u8; 4]>>,
    pub(crate) tangent_sign: f32,
    pub(crate) second_uvs: Option<Vec<[f32; 2]>>,
    pub(crate) bone_map: Option<BoneMap>,
    pub(crate) node_details: Vec<NodeDetails>,
    lods: Vec<Self>,
    extra_chunks: Vec<(u32, Vec<u8>)>,
    geometry_chunks: Vec<(u32, Vec<u8>)>,
    single_file: bool,
    helpers_only: bool,
}

/// The translation of the cube's geometry node relative to its root, in metres, Z up.
pub const CUBE_MESH_OFFSET: [f32; 3] = [10.0, 0.0, 0.0];
/// The material ID of the cube's top and bottom faces.
pub const CUBE_CAPS_MATERIAL: u16 = 1;
/// The material ID of the cube's four sides.
pub const CUBE_SIDES_MATERIAL: u16 = 2;
/// The material library the cube names.
pub const CUBE_MATERIAL_LIBRARY: &str = "objects/test/cube";

impl TestModel {
    /// Returns the test cube with quantized positions and float normals.
    #[must_use]
    pub fn cube() -> Self {
        Self {
            nodes: cube_nodes(),
            vertices: cube_vertices(),
            indices: (0..36).map(cube_index).collect(),
            submeshes: cube_submeshes(),
            material_library: CUBE_MATERIAL_LIBRARY.to_owned(),
            positions: TestPositions::Quantized,
            normals: TestNormals::Float,
            wide_indices: false,
            model_box: None,
            repeated_index_streams: 0,
            colours: None,
            tangent_sign: 1.0,
            second_uvs: None,
            bone_map: None,
            node_details: Vec::new(),
            lods: Vec::new(),
            extra_chunks: Vec::new(),
            geometry_chunks: Vec::new(),
            single_file: false,
            helpers_only: false,
        }
    }

    /// Stores the positions as `positions`.
    #[must_use]
    pub const fn with_positions(mut self, positions: TestPositions) -> Self {
        self.positions = positions;
        self
    }

    /// Stores the normals as `normals`.
    #[must_use]
    pub const fn with_normals(mut self, normals: TestNormals) -> Self {
        self.normals = normals;
        self
    }

    /// Stores the indices as 32-bit values instead of 16-bit ones.
    #[must_use]
    pub const fn with_wide_indices(mut self) -> Self {
        self.wide_indices = true;
        self
    }

    /// Sets the texture coordinate of the vertex `vertex` in stream order.
    #[must_use]
    pub fn with_uv(mut self, vertex: usize, uv: [f32; 2]) -> Self {
        if let Some(slot) = self.vertices.get_mut(vertex) {
            slot.uv = uv;
        }
        self
    }

    /// Writes `min` and `max` as the first, model bounding box of the mesh chunk; the second,
    /// scaling box stays the extent of the vertices, which the positions are quantised against.
    #[must_use]
    pub const fn with_model_box(mut self, min: [f32; 3], max: [f32; 3]) -> Self {
        self.model_box = Some((min, max));
        self
    }

    /// Writes the index stream a second time after the other streams.
    #[must_use]
    pub const fn with_repeated_indices(mut self) -> Self {
        self.repeated_index_streams = 1;
        self
    }

    /// Stores `colours`, one RGBA colour per vertex in stream order, in the vertex stream.
    #[must_use]
    pub fn with_colours(mut self, colours: Vec<[u8; 4]>) -> Self {
        self.colours = Some(colours);
        self
    }

    /// Stores `uvs`, one texture coordinate per vertex in stream order, as the four-byte
    /// `IVONORMALS2` stream of the second texture coordinates.
    #[must_use]
    pub fn with_second_uvs(mut self, uvs: Vec<[f32; 2]>) -> Self {
        self.second_uvs = Some(uvs);
        self
    }

    /// Names `library` as the material library, with an empty name omitting the chunk.
    #[must_use]
    pub fn with_material_library(mut self, library: impl Into<String>) -> Self {
        self.material_library = library.into();
        self
    }

    /// Stores the tangent frames of a `Compressed` model as mirrored: the bitangent points the
    /// other way and bit 31 of the tangent flips the normal back.
    #[must_use]
    pub const fn with_mirrored_tangents(mut self) -> Self {
        self.tangent_sign = -1.0;
        self
    }

    /// Adds the next level of detail, level 1 for the first call: the geometry of `lod` in
    /// `_lod<level>.cgfm` and a `MeshInfo` chunk, with the chunks added to `lod`, in
    /// `_lod<level>.cgf`; the nodes and the material library of `lod` are not used.
    #[must_use]
    pub fn with_lod(mut self, lod: Self) -> Self {
        self.lods.push(lod);
        self
    }

    /// Moves every vertex by `offset` in node space; the bounding boxes follow.
    #[must_use]
    pub fn moved(mut self, offset: [f32; 3]) -> Self {
        for vertex in &mut self.vertices {
            for (axis, value) in vertex.position.iter_mut().enumerate() {
                *value += offset.get(axis).copied().unwrap_or(0.0);
            }
        }
        self
    }

    /// Scales every vertex by `factor` in node space; the bounding boxes follow.
    #[must_use]
    pub fn scaled(mut self, factor: f32) -> Self {
        for vertex in &mut self.vertices {
            for value in &mut vertex.position {
                *value *= factor;
            }
        }
        self
    }

    /// Replaces the nodes; submeshes keep their node indices.
    #[must_use]
    pub fn with_nodes(mut self, nodes: Vec<TestNode>) -> Self {
        self.nodes = nodes;
        self
    }

    /// Replaces the submeshes.
    #[must_use]
    pub fn with_submeshes(mut self, submeshes: Vec<TestSubmesh>) -> Self {
        self.submeshes = submeshes;
        self
    }

    /// Replaces the index at `position` with `index`.
    #[must_use]
    pub fn with_index(mut self, position: usize, index: u32) -> Self {
        if let Some(slot) = self.indices.get_mut(position) {
            *slot = index;
        }
        self
    }

    /// Adds a chunk of type `type_id` to the `.cgf`, after the known ones.
    #[must_use]
    pub fn with_chunk(mut self, type_id: u32, data: impl Into<Vec<u8>>) -> Self {
        self.extra_chunks.push((type_id, data.into()));
        self
    }

    /// Adds a chunk of type `type_id` to the `.cgfm` geometry companion, after the mesh.
    #[must_use]
    pub fn with_geometry_chunk(mut self, type_id: u32, data: impl Into<Vec<u8>>) -> Self {
        self.geometry_chunks.push((type_id, data.into()));
        self
    }

    /// Writes the mesh into the `.cgf` itself, so the model needs no geometry companion.
    #[must_use]
    pub const fn single_file(mut self) -> Self {
        self.single_file = true;
        self
    }

    /// Leaves the `MeshInfo` chunk out of the `.cgf`, as in a model of helper nodes only that has
    /// no geometry companion.
    #[must_use]
    pub const fn helpers_only(mut self) -> Self {
        self.helpers_only = true;
        self
    }

    /// Returns the vertices in stream order.
    #[must_use]
    pub fn vertices(&self) -> &[TestVertex] {
        &self.vertices
    }

    /// Returns the bytes of the `.cgf` file.
    #[must_use]
    pub fn cgf(&self) -> Vec<u8> {
        let mut file = TestChunkFile::ivo().chunk(EXPORT_FLAGS, IVO_VERSION, [0; 128]);
        if !self.material_library.is_empty() {
            file = file.chunk(MTL_NAME_320, IVO_VERSION, self.material_name_chunk());
        }
        if !self.nodes.is_empty() {
            file = file.chunk(NODE_MESH_COMBO, IVO_VERSION, nmc_chunk(self));
        }
        if !self.helpers_only {
            file = file.chunk(MESH_INFO, IVO_VERSION, [0; 104]);
        }
        if self.single_file {
            file = file.chunk(SKIN_2, IVO_VERSION, skin_chunk(self));
        }
        for (type_id, data) in &self.extra_chunks {
            file = file.chunk(*type_id, IVO_VERSION, data.clone());
        }
        file.bytes()
    }

    /// Returns the bytes of the `.cgfm` geometry companion.
    #[must_use]
    pub fn cgfm(&self) -> Vec<u8> {
        let mut file = TestChunkFile::ivo().chunk(SKIN_2, IVO_VERSION, skin_chunk(self));
        for (type_id, data) in &self.geometry_chunks {
            file = file.chunk(*type_id, IVO_VERSION, data.clone());
        }
        file.chunk(TERMINATOR, IVO_VERSION, [0; 16]).bytes()
    }

    /// Returns the bytes of the `_lod<level>.cgf` file, or `None` without that level: a
    /// `MeshInfo` chunk and the chunks added to that level with [`Self::with_chunk`] or
    /// [`Self::with_skeleton`], as the `_lod<level>.skin` file of a skin has its own skeleton.
    #[must_use]
    pub fn lod_cgf(&self, level: usize) -> Option<Vec<u8>> {
        let lod = self.lod(level)?;
        let mut file = TestChunkFile::ivo().chunk(MESH_INFO, IVO_VERSION, [0; 104]);
        for (type_id, data) in &lod.extra_chunks {
            file = file.chunk(*type_id, IVO_VERSION, data.clone());
        }
        Some(file.bytes())
    }

    /// Returns the bytes of the `_lod<level>.cgfm` file, or `None` without that level.
    #[must_use]
    pub fn lod_cgfm(&self, level: usize) -> Option<Vec<u8>> {
        self.lod(level).map(Self::cgfm)
    }

    /// Returns the tangents the vertices decode to, with the handedness in `w`: the unit tangent
    /// of the frame a `Compressed` model stores.
    #[must_use]
    pub fn tangents(&self) -> Vec<[f32; 4]> {
        let w = self.tangent_sign;
        self.vertices
            .iter()
            .map(|vertex| {
                let (tangent, _) = tangent_frame(vertex.normal);
                [tangent[0], tangent[1], tangent[2], w]
            })
            .collect()
    }

    fn lod(&self, level: usize) -> Option<&Self> {
        self.lods.get(level.checked_sub(1)?)
    }

    /// Returns the data of the `Skin2` chunk alone.
    #[must_use]
    pub fn skin_chunk(&self) -> Vec<u8> {
        skin_chunk(self)
    }

    /// Returns the data of the `NodeMeshCombo` chunk alone.
    #[must_use]
    pub fn nmc_chunk(&self) -> Vec<u8> {
        nmc_chunk(self)
    }

    fn material_name_chunk(&self) -> Vec<u8> {
        let mut data = vec![0; 176];
        let name = self.material_library.as_bytes();
        let len = name.len().min(127);
        if let (Some(target), Some(source)) = (data.get_mut(..len), name.get(..len)) {
            target.copy_from_slice(source);
        }
        data
    }
}

fn cube_index(corner: u32) -> u32 {
    const QUAD: [u32; 6] = [0, 1, 2, 0, 2, 3];
    (corner / 6) * 4 + QUAD.get((corner % 6) as usize).copied().unwrap_or(0)
}
