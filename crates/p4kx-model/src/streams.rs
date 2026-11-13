// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::CorruptChunk;
use crate::bone_map::BoneMapStream;
use crate::mesh_stream_source::{StreamScan, StreamStep};
use crate::second_uv::SecondUvStream;
use crate::vector::{
    dequantise, finite_uv, normal_of_quaternion, normal_of_tangent_frame, normalise,
    tangent_of_quaternion, tangent_of_tangent_frame, unpack_unit_vector,
};

pub(crate) const INDICES: u32 = 0xEECD_C168;
const VERTS_UVS: u32 = 0x9132_9AE9;
const VERTS_UVS_2: u32 = 0xB3A7_0D5E;
const NORMALS: u32 = 0x9CF3_F615;
const NORMALS_2: u32 = 0x38A5_81FE;
const TANGENTS: u32 = 0xB95E_9A1B;
const QTANGENTS: u32 = 0xEE05_7252;
const BONEMAP: u32 = 0x677C_7B23;
const BONEMAP32: u32 = 0x6ECA_3708;

const COLOUR_FROM_END: usize = 8;

/// The vertex positions and texture coordinates of a mesh.
#[derive(Debug, Clone, Copy)]
pub(crate) struct VertexStream<'a> {
    data: &'a [u8],
    layout: VertexLayout,
}

#[derive(Debug, Clone, Copy)]
enum VertexLayout {
    Quantised { min: [f32; 3], max: [f32; 3] },
    Float,
}

impl VertexStream<'_> {
    const fn stride(&self) -> usize {
        match self.layout {
            VertexLayout::Quantised { .. } => 16,
            VertexLayout::Float => 20,
        }
    }

    /// Returns the position and texture coordinate of vertex `index`, with a non-finite texture
    /// coordinate read as 0, and whether one was.
    pub(crate) fn vertex(&self, index: usize) -> Result<([f32; 3], [f32; 2], bool), CorruptChunk> {
        let stride = self.stride();
        let bytes = element(self.data, index, stride, "vertex")?;
        let word = |at: usize| u32_at(bytes, at);
        let position = match self.layout {
            VertexLayout::Quantised { min, max } => {
                let raw = [0, 2, 4].map(|at| i16_at(bytes, at));
                dequantise(raw, min, max)
            }
            VertexLayout::Float => [0, 4, 8].map(|at| f32::from_bits(word(at))),
        };
        let uv_at = stride - 4;
        let (uv, repaired) = finite_uv([uv_at, uv_at + 2].map(|at| u16_at(bytes, at)));
        if position.iter().any(|value| !value.is_finite()) {
            return Err(CorruptChunk::NonFinite("vertex position"));
        }
        Ok((position, uv, repaired))
    }

    /// Returns the colour of vertex `index` as red, green, blue and alpha.
    pub(crate) fn colour(&self, index: usize) -> Result<[u8; 4], CorruptChunk> {
        let stride = self.stride();
        let bytes = element(self.data, index, stride, "vertex")?;
        let [blue, green, red, alpha] = std::array::from_fn(|at| {
            bytes
                .get(stride - COLOUR_FROM_END + at)
                .copied()
                .unwrap_or(u8::MAX)
        });
        Ok([red, green, blue, alpha])
    }
}

/// The vertex normals of a mesh in one of their stored forms.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NormalStream<'a> {
    data: &'a [u8],
    kind: NormalKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NormalKind {
    Float,
    Packed,
    TangentFrame,
    Quaternion,
}

impl NormalKind {
    const fn stride(self) -> usize {
        match self {
            Self::Float => 12,
            Self::Packed => 4,
            Self::TangentFrame | Self::Quaternion => 8,
        }
    }

    /// Returns whether the stream stores a tangent frame whose tangent is verified.
    const fn has_tangent(self) -> bool {
        matches!(self, Self::TangentFrame | Self::Quaternion)
    }

    /// The preference between the normal sources of one mesh. Equal ranks keep the stream read
    /// first, so a mesh with `QTANGENTS` and 8-byte `TANGENTS` takes normal and tangent from the
    /// same frame.
    const fn rank(self) -> u8 {
        match self {
            Self::Float => 3,
            Self::Packed => 2,
            Self::TangentFrame | Self::Quaternion => 1,
        }
    }
}

impl NormalStream<'_> {
    /// Returns the unit normal of vertex `index`.
    pub(crate) fn normal(&self, index: usize) -> Result<[f32; 3], CorruptChunk> {
        let bytes = element(self.data, index, self.kind.stride(), "normal")?;
        let word = |at: usize| u32_at(bytes, at);
        let normal = match self.kind {
            NormalKind::Float => normalise([0, 4, 8].map(|at| f32::from_bits(word(at)))),
            NormalKind::Packed => unpack_unit_vector(word(0)),
            NormalKind::TangentFrame => normal_of_tangent_frame(word(0), word(4)),
            NormalKind::Quaternion => {
                normal_of_quaternion([0, 2, 4, 6].map(|at| i16_at(bytes, at)))
            }
        };
        if normal.iter().any(|value| !value.is_finite()) {
            return Err(CorruptChunk::NonFinite("normal"));
        }
        Ok(normal)
    }

    /// Returns the unit tangent of vertex `index` and the handedness of its frame, +1 or −1, or
    /// `None` when the stream stores no tangent frame.
    pub(crate) fn tangent(&self, index: usize) -> Result<Option<[f32; 4]>, CorruptChunk> {
        if !self.kind.has_tangent() {
            return Ok(None);
        }
        let bytes = element(self.data, index, self.kind.stride(), "tangent")?;
        let (tangent, handedness) = match self.kind {
            NormalKind::TangentFrame => tangent_of_tangent_frame(u32_at(bytes, 0)),
            _ => tangent_of_quaternion([0, 2, 4, 6].map(|at| i16_at(bytes, at))),
        };
        if tangent.iter().any(|value| !value.is_finite()) {
            return Err(CorruptChunk::NonFinite("tangent"));
        }
        let [x, y, z] = tangent;
        Ok(Some([x, y, z, handedness]))
    }
}

/// The index list of a mesh, 16 or 32 bits per index.
#[derive(Debug, Clone, Copy)]
pub(crate) struct IndexStream<'a> {
    data: &'a [u8],
    wide: bool,
}

impl IndexStream<'_> {
    pub(crate) const fn from_bytes(data: &[u8], wide: bool) -> IndexStream<'_> {
        IndexStream { data, wide }
    }

    pub(crate) fn source_vertex(&self, at: usize, page_base: u32) -> Result<u64, CorruptChunk> {
        self.index(at)
            .map(|word| Self::apply_page_base(word, page_base))
    }
    pub(crate) const fn apply_page_base(word: u32, page_base: u32) -> u64 {
        word as u64 + page_base as u64
    }

    /// Returns index `position` of the list.
    pub(crate) fn index(&self, position: usize) -> Result<u32, CorruptChunk> {
        if self.wide {
            element(self.data, position, 4, "index").map(|bytes| u32_at(bytes, 0))
        } else {
            element(self.data, position, 2, "index").map(|bytes| u32::from(u16_at(bytes, 0)))
        }
    }
}

/// The data streams of a mesh chunk that a model reads.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Streams<'a> {
    pub(crate) vertices: VertexStream<'a>,
    pub(crate) normals: Option<NormalStream<'a>>,
    pub(crate) tangents: Option<NormalStream<'a>>,
    pub(crate) second_uvs: Option<SecondUvStream<'a>>,
    pub(crate) bone_map: Option<BoneMapStream<'a>>,
    pub(crate) indices: IndexStream<'a>,
}

/// The counts and bounding box the streams are read with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StreamCounts {
    pub(crate) vertices: u32,
    pub(crate) indices: u32,
    pub(crate) min: [f32; 3],
    pub(crate) max: [f32; 3],
}

/// Reads the streams that follow the submeshes until the end of the chunk; each is a type tag,
/// an element size and one element per vertex or index, padded to 8 bytes.
pub(crate) fn read_streams<'a>(
    reader: ByteReader<'a>,
    counts: StreamCounts,
) -> Result<Streams<'a>, CorruptChunk> {
    let mut vertices = None;
    let mut normals: Option<NormalStream<'a>> = None;
    let mut tangents: Option<NormalStream<'a>> = None;
    let mut second_uvs = None;
    let mut bone_map: Option<BoneMapStream<'a>> = None;
    let mut indices = None;
    let mut seen = 0_u16;
    let scan = StreamScan::new(reader, counts.vertices, counts.indices);
    for step in scan {
        let (tag, size, data) = match step? {
            StreamStep::Complete(stream) => (
                stream.header().tag(),
                stream.header().stride(),
                stream.bytes(),
            ),
            StreamStep::Incomplete(stream, error) => {
                if repeat(&mut seen, stream.tag()) {
                    return Err(CorruptChunk::RepeatedStream(stream.tag()));
                }
                if stream.is_recognized() {
                    return Err(error.into());
                }
                break;
            }
        };
        if repeat(&mut seen, tag) {
            return Err(CorruptChunk::RepeatedStream(tag));
        }
        match classify(tag, size, data, counts)? {
            Some(Found::Indices(stream)) => indices = Some(stream),
            Some(Found::Vertices(stream)) => vertices = Some(stream),
            Some(Found::Normals(stream)) => {
                if stream.kind.has_tangent() && tangents.is_none() {
                    tangents = Some(stream);
                }
                if normals.is_none_or(|known| known.kind.rank() < stream.kind.rank()) {
                    normals = Some(stream);
                }
            }
            Some(Found::SecondUvs(stream)) => second_uvs = Some(stream),
            Some(Found::BoneMap(stream))
                if bone_map.is_none_or(|known| !known.is_wide() && stream.is_wide()) =>
            {
                bone_map = Some(stream);
            }
            Some(Found::BoneMap(_)) | None => {}
        }
    }
    Ok(Streams {
        vertices: vertices.ok_or(CorruptChunk::MissingStream("vertex"))?,
        normals,
        tangents,
        second_uvs,
        bone_map,
        indices: indices.ok_or(CorruptChunk::MissingStream("index"))?,
    })
}

const fn repeat(seen: &mut u16, tag: u32) -> bool {
    let Some(bit) = read_tag_bit(tag) else {
        return false;
    };
    let repeated = *seen & bit != 0;
    *seen |= bit;
    repeated
}

pub(crate) const fn recognized(tag: u32, size: u32) -> bool {
    matches!(tag, INDICES | VERTS_UVS | VERTS_UVS_2 | BONEMAP | BONEMAP32)
        || (tag == NORMALS_2 && size == 4)
        || normal_kind(tag, size).is_some()
}

const fn read_tag_bit(tag: u32) -> Option<u16> {
    match tag {
        INDICES => Some(1),
        VERTS_UVS => Some(1 << 1),
        VERTS_UVS_2 => Some(1 << 2),
        NORMALS => Some(1 << 3),
        NORMALS_2 => Some(1 << 4),
        TANGENTS => Some(1 << 5),
        QTANGENTS => Some(1 << 6),
        BONEMAP => Some(1 << 7),
        BONEMAP32 => Some(1 << 8),
        _ => None,
    }
}

enum Found<'a> {
    Indices(IndexStream<'a>),
    Vertices(VertexStream<'a>),
    Normals(NormalStream<'a>),
    SecondUvs(SecondUvStream<'a>),
    BoneMap(BoneMapStream<'a>),
}

fn classify(
    tag: u32,
    size: u32,
    data: &[u8],
    counts: StreamCounts,
) -> Result<Option<Found<'_>>, CorruptChunk> {
    let (min, max) = (counts.min, counts.max);
    Ok(match (tag, size) {
        (INDICES, 2 | 4) => Some(Found::Indices(IndexStream {
            data,
            wide: size == 4,
        })),
        (VERTS_UVS | VERTS_UVS_2, 16) => Some(Found::Vertices(VertexStream {
            data,
            layout: VertexLayout::Quantised { min, max },
        })),
        (VERTS_UVS | VERTS_UVS_2, 20) => Some(Found::Vertices(VertexStream {
            data,
            layout: VertexLayout::Float,
        })),
        (NORMALS_2, 4) => Some(Found::SecondUvs(SecondUvStream::new(data))),
        (BONEMAP, 12) | (BONEMAP32, 24) => {
            Some(Found::BoneMap(BoneMapStream::new(data, tag == BONEMAP32)))
        }
        (INDICES | VERTS_UVS | VERTS_UVS_2 | BONEMAP | BONEMAP32, _) => {
            return Err(CorruptChunk::ElementSize {
                stream: name(tag),
                size,
            });
        }
        _ => normal_kind(tag, size).map(|kind| Found::Normals(NormalStream { data, kind })),
    })
}
const fn normal_kind(tag: u32, size: u32) -> Option<NormalKind> {
    match (tag, size) {
        (NORMALS | NORMALS_2, 12) => Some(NormalKind::Float),
        (NORMALS, 4) => Some(NormalKind::Packed),
        (QTANGENTS, 8) => Some(NormalKind::TangentFrame),
        (TANGENTS, 8) => Some(NormalKind::Quaternion),
        _ => None,
    }
}

const fn name(tag: u32) -> &'static str {
    match tag {
        INDICES => "index",
        VERTS_UVS | VERTS_UVS_2 => "vertex",
        BONEMAP | BONEMAP32 => "bone map",
        _ => "data",
    }
}

pub(crate) fn element<'a>(
    data: &'a [u8],
    index: usize,
    stride: usize,
    stream: &'static str,
) -> Result<&'a [u8], CorruptChunk> {
    index
        .checked_mul(stride)
        .and_then(|start| data.get(start..start.checked_add(stride)?))
        .ok_or(CorruptChunk::MissingStream(stream))
}

pub(crate) fn u16_at(bytes: &[u8], at: usize) -> u16 {
    let pair = bytes
        .get(at..at + 2)
        .and_then(|pair| <[u8; 2]>::try_from(pair).ok());
    pair.map_or(0, u16::from_le_bytes)
}

fn i16_at(bytes: &[u8], at: usize) -> i16 {
    i16::from_le_bytes(u16_at(bytes, at).to_le_bytes())
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    let word = bytes
        .get(at..at + 4)
        .and_then(|word| <[u8; 4]>::try_from(word).ok());
    word.map_or(0, u32::from_le_bytes)
}
