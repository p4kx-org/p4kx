// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::mesh_chunk_source::{MeshHeader, SUBMESH_BYTES, read_submesh};
use crate::reader::to_usize;
use crate::streams::{StreamCounts, Streams, read_streams};
use crate::{CorruptChunk, PositionBox};

/// One submesh as the mesh chunk stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RawSubmesh {
    pub(crate) material_id: u16,
    pub(crate) node: u16,
    pub(crate) first_index: u32,
    pub(crate) index_count: u32,
    pub(crate) page_base: u32,
}

/// A `Skin` or `Skin2` mesh chunk: its counts, submeshes and data streams, borrowed from the chunk.
#[derive(Debug, Clone)]
pub(crate) struct SkinMesh<'a> {
    pub(crate) vertex_count: u32,
    pub(crate) index_count: u32,
    pub(crate) submeshes: Vec<RawSubmesh>,
    pub(crate) streams: Streams<'a>,
}

/// Reads a mesh chunk: a flags word, the mesh info with counts and bounding boxes, padding, the
/// submeshes, extra words and the data streams.
///
/// Quantised positions are dequantised with the box `position_box` picks: the second, scaling box
/// by default. Only static models are quantised in the real Archives; where the face normals can
/// tell, the second box is the one that fits (docs/mesh-bounding-boxes.md).
pub(crate) fn read_skin(
    data: &[u8],
    position_box: PositionBox,
) -> Result<SkinMesh<'_>, CorruptChunk> {
    let mut reader = ByteReader::new(data);
    let header = MeshHeader::read(&mut reader)?;
    let (min, max) = match position_box {
        PositionBox::Model => header.model_box,
        PositionBox::Scaling => header.scaling_box,
    };
    if min.iter().chain(&max).any(|value| !value.is_finite()) {
        return Err(CorruptChunk::NonFinite("bounding box"));
    }
    let submesh_count = to_usize(header.counts.submeshes);
    let mut submeshes = Vec::with_capacity(submesh_count.min(reader.remaining() / SUBMESH_BYTES));
    for _ in 0..submesh_count {
        submeshes.push(read_submesh(&mut reader)?);
    }
    reader.skip(to_usize(header.counts.extra_words).saturating_mul(size_of::<u32>()))?;
    let counts = StreamCounts {
        vertices: header.counts.vertices,
        indices: header.counts.indices,
        min,
        max,
    };
    let streams = read_streams(reader, counts)?;
    Ok(SkinMesh {
        vertex_count: header.counts.vertices,
        index_count: header.counts.indices,
        submeshes,
        streams,
    })
}
