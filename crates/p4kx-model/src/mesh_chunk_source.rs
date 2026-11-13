// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{Chunk, ivo};
use p4kx_foundation::{ByteReader, ParseError};

use crate::CorruptChunk;
use crate::mesh_stream_source::{MeshStreamOccurrence, StreamScan, StreamStep};
use crate::reader::{ReadValues, to_usize};
use crate::skin::RawSubmesh;
use crate::streams::IndexStream;

const HEADER_PADDING: usize = 88;
const WORD_BYTES: u64 = size_of::<u32>() as u64;
pub(crate) const SUBMESH_BYTES: usize = 48;

pub(crate) struct MeshHeader {
    pub(crate) counts: MeshCounts,
    pub(crate) model_box: ([f32; 3], [f32; 3]),
    pub(crate) scaling_box: ([f32; 3], [f32; 3]),
}

impl MeshHeader {
    pub(crate) fn read(reader: &mut ByteReader<'_>) -> Result<Self, ParseError> {
        reader.skip(2 * size_of::<u32>())?;
        let vertices = reader.read_u32()?;
        let indices = reader.read_u32()?;
        let submeshes = reader.read_u32()?;
        reader.skip(size_of::<u32>())?;
        let model_box = (reader.read_vec3()?, reader.read_vec3()?);
        let scaling_box = (reader.read_vec3()?, reader.read_vec3()?);
        reader.skip(size_of::<u32>())?;
        let extra_words = reader.read_u32()?;
        reader.skip(HEADER_PADDING)?;
        Ok(Self {
            counts: MeshCounts {
                vertices,
                indices,
                submeshes,
                extra_words,
            },
            model_box,
            scaling_box,
        })
    }
}

pub(crate) fn read_submesh(reader: &mut ByteReader<'_>) -> Result<RawSubmesh, CorruptChunk> {
    let material_id = reader.read_u16()?;
    let node = reader.read_u16()?;
    let first_index = reader.read_u32()?;
    let index_count = reader.read_u32()?;
    reader.skip(size_of::<u32>())?;
    let page_base = reader.read_u32()?;
    reader.skip(SUBMESH_BYTES - 20)?;
    Ok(RawSubmesh {
        material_id,
        node,
        first_index,
        index_count,
        page_base,
    })
}

/// Counts in the original IVO mesh header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MeshCounts {
    /// Number of entries in the source vertex stream.
    pub vertices: u32,
    /// Number of entries in the source index stream.
    pub indices: u32,
    /// Number of physical submesh declarations.
    pub submeshes: u32,
    /// Number of extra words between the submesh rows and streams.
    pub extra_words: u32,
}

/// A borrowed source mesh with physical submeshes and stream locations.
#[derive(Debug)]
pub struct MeshChunkSource<'input> {
    boxes: [[[u32; 3]; 2]; 2],
    counts: MeshCounts,
    submeshes: &'input [[u8; SUBMESH_BYTES]],
    submeshes_at: u64,
    streams: &'input [u8],
    streams_at: u64,
}

impl<'input> MeshChunkSource<'input> {
    /// Returns whether the physical chunk uses the version 0x900 IVO Skin layout.
    #[must_use]
    pub const fn supports(chunk: &Chunk) -> bool {
        matches!(chunk.chunk_type(), ivo::SKIN | ivo::SKIN_2) && chunk.version() == 0x900
    }

    /// Admits a bounded mesh header and its physical submesh rows from one selected chunk.
    ///
    /// Returns absent for other original chunk types or versions.
    ///
    /// # Errors
    /// Returns an error when the header, submesh declarations or intervening words are truncated.
    pub fn read(chunk: &Chunk, bytes: &'input [u8]) -> Result<Option<Self>, CorruptChunk> {
        if !Self::supports(chunk) {
            return Ok(None);
        }
        let size = usize::try_from(chunk.size()).unwrap_or(usize::MAX);
        let available = bytes.get(..size).ok_or_else(|| ParseError::UnexpectedEnd {
            offset: chunk.offset(),
            needed: chunk.size(),
            available: bytes.len() as u64,
        })?;
        let mut reader = ByteReader::with_base(available, chunk.offset());
        let header = MeshHeader::read(&mut reader)?;
        let submeshes_at = reader.offset();
        let submeshes = reader
            .read_bytes(to_usize(header.counts.submeshes).saturating_mul(SUBMESH_BYTES))?
            .as_chunks()
            .0;
        reader.skip(to_usize(header.counts.extra_words).saturating_mul(size_of::<u32>()))?;
        let streams_at = reader.offset();
        let streams = reader.read_bytes(reader.remaining())?;
        Ok(Some(Self {
            boxes: [
                [
                    header.model_box.0.map(f32::to_bits),
                    header.model_box.1.map(f32::to_bits),
                ],
                [
                    header.scaling_box.0.map(f32::to_bits),
                    header.scaling_box.1.map(f32::to_bits),
                ],
            ],
            counts: header.counts,
            submeshes,
            submeshes_at,
            streams_at,
            streams,
        }))
    }

    /// Returns the original mesh counts.
    #[must_use]
    pub const fn counts(&self) -> MeshCounts {
        self.counts
    }

    /// Returns the stored model and scaling box binary32 words as min and max XYZ pairs.
    #[must_use]
    pub const fn bounds_bits(&self) -> [[[u32; 3]; 2]; 2] {
        self.boxes
    }

    /// Returns all source submesh ranges and fields, including declarations with zero triangles.
    pub fn submeshes(&self) -> impl ExactSizeIterator<Item = MeshSubmeshSource> + '_ {
        self.submeshes.iter().enumerate().map(|(ordinal, row)| {
            MeshSubmeshSource::new(
                ordinal,
                self.submeshes_at + ordinal as u64 * SUBMESH_BYTES as u64,
                row,
            )
        })
    }

    /// Returns the original stream walker without an allocation per displayed vertex.
    pub fn streams(
        &self,
    ) -> impl Iterator<Item = Result<MeshStreamOccurrence<'input>, CorruptChunk>> + use<'input>
    {
        let reader = ByteReader::with_base(self.streams, self.streams_at);
        StreamScan::new(reader, self.counts.vertices, self.counts.indices)
            .map(|step| step.map(StreamStep::disclosed))
    }
}

/// Direct fields of one original IVO submesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshSubmeshSource {
    ordinal: usize,
    start: u64,
    inner: RawSubmesh,
}

impl MeshSubmeshSource {
    fn new(ordinal: usize, start: u64, row: &[u8; SUBMESH_BYTES]) -> Self {
        // A fixed-width row admits every known field before iteration.
        #[expect(
            clippy::expect_used,
            reason = "the caller supplies exactly one complete submesh row"
        )]
        let inner = read_submesh(&mut ByteReader::new(row)).expect("fixed-width submesh row");
        Self {
            ordinal,
            start,
            inner,
        }
    }

    /// Returns the original row ordinal before model grouping and filtering.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }

    /// Returns the material ID from the physical row.
    #[must_use]
    pub const fn material(self) -> u16 {
        self.inner.material_id
    }

    /// Returns the original node or skin-zone ID from the physical row.
    #[must_use]
    pub const fn node(self) -> u16 {
        self.inner.node
    }

    /// Returns the first source index named by the row.
    #[must_use]
    pub const fn first_index(self) -> u32 {
        self.inner.first_index
    }

    /// Returns the declared source index count of the row.
    #[must_use]
    pub const fn index_count(self) -> u32 {
        self.inner.index_count
    }

    /// Returns the original source vertex base added to indices belonging to the row.
    #[must_use]
    pub const fn page_base(self) -> u32 {
        self.inner.page_base
    }

    /// Returns the complete original submesh row's byte interval.
    #[must_use]
    pub const fn range(self) -> Range<u64> {
        self.start..self.start + SUBMESH_BYTES as u64
    }

    /// Returns the index-stream selection and page-base words from the original row.
    #[must_use]
    pub const fn index_selection_range(self) -> Range<u64> {
        self.start + WORD_BYTES..self.start + 3 * WORD_BYTES
    }

    /// Returns the original source vertex page-base word's byte interval.
    #[must_use]
    pub const fn page_base_range(self) -> Range<u64> {
        self.start + 4 * WORD_BYTES..self.start + 5 * WORD_BYTES
    }

    /// Streams the physical indices this submesh names, with page-base resolved source vertices.
    ///
    /// # Errors
    /// Returns an error for an unsupported index stride, an out-of-range submesh or a vertex outside
    /// the source vertex stream.
    pub fn vertices(
        self,
        stream: crate::MeshStreamSource<'_>,
        counts: MeshCounts,
    ) -> Result<
        impl ExactSizeIterator<Item = Result<MeshVertexReference, CorruptChunk>> + '_,
        CorruptChunk,
    > {
        let header = stream.header();
        if !header.is_index_stream() || !matches!(header.stride(), 2 | 4) {
            return Err(CorruptChunk::ElementSize {
                stream: "index",
                size: header.stride(),
            });
        }
        super::mesh::check_range(self.ordinal, &self.inner, header.count())?;
        let packed = IndexStream::from_bytes(stream.bytes(), header.stride() == 4);
        let first = to_usize(self.inner.first_index);
        let end = first + to_usize(self.inner.index_count);
        let base = stream.payload_range().start;
        let width = u64::from(header.stride());
        Ok((first..end).map(move |at| {
            let raw = packed.index(at)?;
            let vertex = IndexStream::apply_page_base(raw, self.inner.page_base);
            if vertex >= u64::from(counts.vertices) {
                return Err(CorruptChunk::IndexRange {
                    submesh: self.ordinal,
                    index: vertex,
                    vertices: counts.vertices,
                });
            }
            let start = base + at as u64 * width;
            Ok(MeshVertexReference {
                index_ordinal: at,
                raw,
                source_vertex: vertex,
                range: start..start + width,
            })
        }))
    }
}

/// One original index word before and after adding the authored page base.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MeshVertexReference {
    /// Physical index ordinal in the original chunk.
    pub index_ordinal: usize,
    /// Unmodified u16/u32 word as stored.
    pub raw: u32,
    /// Source vertex ordinal after applying the original submesh's page base.
    pub source_vertex: u64,
    /// Exact entry-relative interval of the original index word.
    pub range: Range<u64>,
}

#[cfg(test)]
#[path = "mesh_chunk_source_tests.rs"]
mod tests;
