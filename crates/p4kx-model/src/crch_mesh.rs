// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::error::ChunkFailure;
use crate::reader::ReadValues;
use crate::smooth_normals::smooth_normals;
use crate::{CorruptChunk, Mesh, Submesh, UnsupportedLayout};

const STREAM_SLOTS: usize = 16;
const PHYSICS_SLOTS: usize = 4;
const PHYSICS_HEADER_LEN: usize = 24;
const PRIMITIVE_AT: usize = 0x44;
const POLYHEDRON: u32 = 1;
const COUNTS_AT: usize = 0x48;
const VERTICES_AT: usize = 0x5A;
const VERTEX_LEN: u64 = 12;
const INDEX_LEN: u64 = 2;

/// The physics data a `Mesh` header names: the first slot that names any, and how many further
/// slots name some that are not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhysicsSlots {
    pub(crate) chunk_id: Option<u32>,
    pub(crate) ignored: usize,
}

/// Reads the header of a version `0x801` `Mesh` chunk and returns the physics data it names.
///
/// The header declares counts and chunk IDs of data streams; the meshes of the Archives declare
/// none and keep their geometry in the physics data, so a header that declares any is refused as
/// unsupported. Of the four physics slots (default, no-collide, obstruct, proxy) the first that
/// names a chunk is read; the Archives fill only the default slot.
pub(crate) fn physics_slots(data: &[u8]) -> Result<PhysicsSlots, ChunkFailure> {
    let mut reader = ByteReader::new(data);
    reader.skip(8)?;
    let vertices = reader.read_u32()?;
    let indices = reader.read_u32()?;
    let subsets = reader.read_u32()?;
    let subsets_chunk = reader.read_u32()?;
    reader.skip(4)?;
    let mut streams = 0;
    for _ in 0..STREAM_SLOTS {
        streams |= reader.read_u32()?;
    }
    if (vertices | indices | subsets | subsets_chunk | streams) != 0 {
        return Err(UnsupportedLayout::StreamedMesh { vertices, indices }.into());
    }
    let mut slots = PhysicsSlots {
        chunk_id: None,
        ignored: 0,
    };
    for _ in 0..PHYSICS_SLOTS {
        let id = reader.read_u32()?;
        if id == 0 {
            continue;
        }
        if slots.chunk_id.is_none() {
            slots.chunk_id = Some(id);
        } else {
            slots.ignored += 1;
        }
    }
    Ok(slots)
}

/// Reads the triangle mesh of a version `0x800` `MeshPhysicsData` chunk as the mesh of `node`,
/// or `None` when it has no triangles.
///
/// The chunk is a 24-byte header whose first word is the length of the physics data, then the
/// data: the primitive type at byte `0x44`, which must be 1 (a polyhedron), the number of
/// vertices and of triangles at byte `0x48`, the
/// vertices as three `f32` each from byte `0x5A`, then the triangles as three `u16` vertex
/// indices each. A triangle has no material ID of its own that is read; the mesh is one submesh
/// of material ID 0, and it has no normals or texture coordinates.
pub(crate) fn read_physics_mesh(node: usize, chunk: &[u8]) -> Result<Option<Mesh>, ChunkFailure> {
    let mut header = ByteReader::new(chunk);
    let len = usize::try_from(header.read_u32()?).unwrap_or(usize::MAX);
    header.skip(PHYSICS_HEADER_LEN - 4)?;
    let mut reader = ByteReader::new(header.read_bytes(len)?);
    reader.skip(PRIMITIVE_AT)?;
    let primitive = reader.read_u32()?;
    if primitive != POLYHEDRON {
        return Err(UnsupportedLayout::PhysicsPrimitive(primitive).into());
    }
    reader.skip(COUNTS_AT - PRIMITIVE_AT - 4)?;
    let vertex_count = reader.read_u32()?;
    let triangle_count = reader.read_u32()?;
    reader.skip(VERTICES_AT - COUNTS_AT - 8)?;
    let needed = u64::from(vertex_count) * VERTEX_LEN + u64::from(triangle_count) * 3 * INDEX_LEN;
    if needed > reader.remaining() as u64 {
        return Err(CorruptChunk::PhysicsCounts {
            vertices: vertex_count,
            triangles: triangle_count,
            len,
        }
        .into());
    }
    if triangle_count == 0 {
        return Ok(None);
    }

    let mut positions = Vec::with_capacity(crate::reader::to_usize(vertex_count));
    for _ in 0..vertex_count {
        let position = reader.read_vec3()?;
        if position.iter().any(|value| !value.is_finite()) {
            return Err(CorruptChunk::NonFinite("physics mesh vertex").into());
        }
        positions.push(position);
    }
    let index_count = crate::reader::to_usize(triangle_count) * 3;
    let mut indices = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        let index = u32::from(reader.read_u16()?);
        if index >= vertex_count {
            return Err(CorruptChunk::PhysicsIndex {
                index,
                vertices: vertex_count,
            }
            .into());
        }
        indices.push(index);
    }
    Ok(Some(bare_mesh(node, positions, indices)))
}

fn bare_mesh(node: usize, positions: Vec<[f32; 3]>, indices: Vec<u32>) -> Mesh {
    Mesh {
        origin: None,
        index_width: None,
        original_vertex_order_changed: false,
        node,
        source: (0..u32::try_from(positions.len()).unwrap_or(u32::MAX)).collect(),
        submeshes: vec![Submesh {
            material_id: 0,
            node_id: 0,
            first_index: 0,
            index_count: indices.len(),
            origin: None,
        }],
        normals: (!indices.is_empty()).then(|| smooth_normals(&positions, &indices)),
        normals_computed: !indices.is_empty(),
        positions,
        tangents: None,
        colours: None,
        influences: None,
        uvs: Vec::new(),
        second_uvs: None,
        non_finite_uvs: 0,
        indices,
    }
}
