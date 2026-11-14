// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{ByteOrder, Chunk, ChunkFile, ChunkType, Container};
use p4kx_foundation::ByteReader;

use crate::included::{self, IncludedObjectsError};
use crate::included_object_row::IncludedObjectRows;
use crate::reader;

const INCLUDED_OBJECTS: ChunkType = ChunkType::new(Container::CrCh, 0x0010);
const PATH_LEN: usize = 256;
const PREFIX_LEN: usize = 4;
const LAYOUT_METADATA_LEN: usize = 24;

/// One original `IncludedObjects` chunk in the SOC chunk table.
#[derive(Debug, Clone, Copy)]
pub struct IncludedObjectsChunk<'file> {
    chunk_index: usize,
    header: &'file Chunk,
    bytes: &'file [u8],
}

impl<'file> IncludedObjectsChunk<'file> {
    /// Returns the original zero-based chunk table index.
    #[must_use]
    pub const fn chunk_index(self) -> usize {
        self.chunk_index
    }

    /// Returns the original chunk header and its file-relative payload extent.
    #[must_use]
    pub const fn header(self) -> &'file Chunk {
        self.header
    }

    /// Borrows the original chunk payload.
    #[must_use]
    pub const fn bytes(self) -> &'file [u8] {
        self.bytes
    }

    /// Positions the original path tables and object records without allocating rows.
    ///
    /// # Errors
    /// Returns an error when the tables cannot be bounded or the byte order is unsupported.
    pub fn tables(self) -> Result<IncludedObjectTables<'file>, IncludedObjectsError> {
        self.read_tables(None)
    }

    pub(crate) fn read_tables(
        self,
        model_limit: Option<usize>,
    ) -> Result<IncludedObjectTables<'file>, IncludedObjectsError> {
        if self.header.byte_order() == ByteOrder::Big {
            return Err(IncludedObjectsError::BigEndian);
        }
        IncludedObjectTables::read(self.bytes, model_limit)
    }
}

pub(crate) fn chunks<B: AsRef<[u8]>>(
    file: &ChunkFile<B>,
) -> impl Iterator<Item = IncludedObjectsChunk<'_>> {
    file.chunks_of(INCLUDED_OBJECTS)
        .map(|(chunk_index, header)| {
            // The parsed chunk table validates every payload extent.
            let bytes = file.chunk_data(chunk_index).unwrap_or_default();
            IncludedObjectsChunk {
                chunk_index,
                header,
                bytes,
            }
        })
}

/// Bounded original tables in an `IncludedObjects` chunk.
#[derive(Debug)]
pub struct IncludedObjectTables<'chunk> {
    models: Paths<'chunk>,
    materials: Paths<'chunk>,
    palettes: Paths<'chunk>,
    layout: u32,
    layout_range: Range<u64>,
    objects: &'chunk [u8],
    object_len_declared: u32,
    objects_offset: u64,
    trailing: &'chunk [u8],
}

impl<'chunk> IncludedObjectTables<'chunk> {
    fn read(data: &'chunk [u8], model_limit: Option<usize>) -> Result<Self, IncludedObjectsError> {
        let mut input = ByteReader::new(data);
        input.skip(PREFIX_LEN)?;
        let model_count = reader::to_usize(input.read_u32()?);
        if let Some(limit) = model_limit {
            included::check(model_count, limit, "models")?;
        }
        let models = Paths::read(&mut input, model_count)?;
        let material_count = usize::from(input.read_u16()?);
        let palette_count = usize::from(input.read_u16()?);
        let materials = Paths::read(&mut input, material_count)?;
        let palettes = Paths::read(&mut input, palette_count)?;
        let layout_start = input.offset();
        let layout = input.read_u32()?;
        let layout_range = layout_start..input.offset();
        input.skip(LAYOUT_METADATA_LEN)?;
        let object_len_declared = input.read_u32()?;
        let objects_offset = input.offset();
        let objects_len = reader::to_usize(object_len_declared).min(input.remaining());
        let objects = input.read_bytes(objects_len)?;
        let trailing = input.read_bytes(input.remaining())?;
        Ok(Self {
            models,
            materials,
            palettes,
            layout,
            layout_range,
            objects,
            object_len_declared,
            objects_offset,
            trailing,
        })
    }

    /// Visits model-path slots in their original table order, including duplicates and empty slots.
    pub fn models(&self) -> impl ExactSizeIterator<Item = IncludedPathSlot<'chunk>> {
        self.models.slots()
    }

    /// Visits material-path slots in their original table order.
    pub fn materials(&self) -> impl ExactSizeIterator<Item = IncludedPathSlot<'chunk>> {
        self.materials.slots()
    }

    /// Visits tint-palette-path slots in their original table order.
    pub fn palettes(&self) -> impl ExactSizeIterator<Item = IncludedPathSlot<'chunk>> {
        self.palettes.slots()
    }

    /// Returns the chunk-relative extent of the model path table.
    #[must_use]
    pub const fn model_range(&self) -> Range<u64> {
        self.models.range()
    }

    /// Returns the chunk-relative extent of the material path table.
    #[must_use]
    pub const fn material_range(&self) -> Range<u64> {
        self.materials.range()
    }

    /// Returns the chunk-relative extent of the palette path table.
    #[must_use]
    pub const fn palette_range(&self) -> Range<u64> {
        self.palettes.range()
    }

    /// Returns the original layout word.
    #[must_use]
    pub const fn layout(&self) -> u32 {
        self.layout
    }

    /// Returns the chunk-relative byte range of the layout word.
    #[must_use]
    pub fn layout_range(&self) -> Range<u64> {
        self.layout_range.clone()
    }

    /// Returns the readable object table's chunk-relative byte range.
    #[must_use]
    pub const fn object_range(&self) -> Range<u64> {
        self.objects_offset..self.objects_offset + self.objects.len() as u64
    }

    /// Returns the original byte count declared for the object table.
    #[must_use]
    pub const fn object_len_declared(&self) -> u32 {
        self.object_len_declared
    }

    /// Borrows the bytes after the declared object table.
    #[must_use]
    pub const fn trailing_bytes(&self) -> &'chunk [u8] {
        self.trailing
    }

    /// Visits positioned records and at most one opaque or truncated tail.
    #[must_use]
    pub const fn rows(&self) -> IncludedObjectRows<'chunk> {
        IncludedObjectRows::new(self.objects, self.objects_offset, self.layout)
    }

    pub(crate) const fn byte_len(&self) -> usize {
        self.objects.len()
    }
}

#[derive(Debug)]
struct Paths<'chunk> {
    bytes: &'chunk [u8],
    offset: u64,
}

impl<'chunk> Paths<'chunk> {
    fn read(input: &mut ByteReader<'chunk>, count: usize) -> Result<Self, IncludedObjectsError> {
        let offset = input.offset();
        let bytes = input.read_bytes(count.saturating_mul(PATH_LEN))?;
        Ok(Self { bytes, offset })
    }

    const fn range(&self) -> Range<u64> {
        self.offset..self.offset + self.bytes.len() as u64
    }

    fn slots(&self) -> impl ExactSizeIterator<Item = IncludedPathSlot<'chunk>> {
        let start = self.offset;
        self.bytes
            .as_chunks::<PATH_LEN>()
            .0
            .iter()
            .enumerate()
            .map(move |(ordinal, bytes)| IncludedPathSlot {
                ordinal,
                offset: start + (ordinal as u64 * PATH_LEN as u64),
                bytes,
            })
    }
}

/// An original fixed-width path slot.
#[derive(Debug, Clone, Copy)]
pub struct IncludedPathSlot<'chunk> {
    ordinal: usize,
    offset: u64,
    bytes: &'chunk [u8; PATH_LEN],
}

impl<'chunk> IncludedPathSlot<'chunk> {
    /// Returns the original index in its local path table.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }

    /// Returns the chunk-relative byte range including the full slot padding.
    #[must_use]
    pub const fn range(self) -> Range<u64> {
        self.offset..self.offset + PATH_LEN as u64
    }

    /// Borrows the entire path slot, including any bytes after the first NUL.
    #[must_use]
    pub const fn bytes(self) -> &'chunk [u8; PATH_LEN] {
        self.bytes
    }

    /// Borrows the path before the first NUL, preserving its original spelling.
    #[must_use]
    pub fn path_bytes(self) -> &'chunk [u8] {
        self.bytes
            .split(|&byte| byte == 0)
            .next()
            .unwrap_or(self.bytes)
    }
}
