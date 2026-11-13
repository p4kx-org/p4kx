// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_foundation::ByteReader;

use crate::{ByteOrder, Chunk, ChunkError, ChunkType, Container};

pub(crate) const HEADER_LEN: usize = 16;
pub(crate) const TABLE_ENTRY_LEN: usize = 16;
const CRCH_BIG_ENDIAN_FLAG: u16 = 0x8000;

/// A parsed chunk file: its container form and its chunk table over the file's bytes.
#[derive(Debug, Clone)]
pub struct ChunkFile<B> {
    bytes: B,
    container: Container,
    chunks: Vec<Chunk>,
    table: Range<u64>,
}

impl<B: AsRef<[u8]>> ChunkFile<B> {
    /// Parses the header and chunk table of `bytes`; chunk data is not interpreted.
    ///
    /// # Errors
    ///
    /// Returns [`ChunkError::UnknownSignature`] or [`ChunkError::UnsupportedVersion`] when the
    /// file is no chunk file this crate reads, and [`ChunkError::Truncated`],
    /// [`ChunkError::TableOutOfBounds`], [`ChunkError::ChunkOutOfBounds`] or
    /// [`ChunkError::ChunkInHeaderOrTable`] when it is truncated or corrupt.
    pub fn parse(bytes: B) -> Result<Self, ChunkError> {
        let data = bytes.as_ref();
        let header = ChunkHeader::read(data, data.len() as u64)?;
        let start = usize::try_from(header.table.start).map_err(|_| header.table_error())?;
        let end = usize::try_from(header.table.end).map_err(|_| header.table_error())?;
        let table = data.get(start..end).ok_or_else(|| header.table_error())?;
        let chunks = read_table(&header, table, || Ok::<_, ChunkError>(()))?;
        Ok(Self {
            bytes,
            container: header.container,
            chunks,
            table: header.table,
        })
    }

    /// Returns the container form.
    #[must_use]
    pub const fn container(&self) -> Container {
        self.container
    }

    /// Returns the chunk table in file order.
    #[must_use]
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }

    /// Returns the byte interval occupied by the validated table.
    #[must_use]
    pub fn table_range(&self) -> Range<u64> {
        self.table.clone()
    }

    /// Returns the source intervals of one original table row.
    #[must_use]
    pub fn table_row(&self, index: usize) -> Option<ChunkTableRow> {
        self.chunks.get(index)?;
        let start = self.table.start + index as u64 * TABLE_ENTRY_LEN as u64;
        Some(ChunkTableRow::at(start, self.container))
    }

    /// Returns the data of the chunk at `index` in the table, or `None` when there is no such chunk.
    #[must_use]
    pub fn chunk_data(&self, index: usize) -> Option<&[u8]> {
        let chunk = self.chunks.get(index)?;
        let start = usize::try_from(chunk.offset).ok()?;
        let end = start.checked_add(usize::try_from(chunk.size).ok()?)?;
        self.bytes.as_ref().get(start..end)
    }

    /// Returns all bytes from the start of the chunk at `index` to the end of the file, or `None`
    /// when there is no such chunk.
    ///
    /// Unlike [`Self::chunk_data`] this is not bounded by the chunk's [`Chunk::size`]: an `#ivo`
    /// `.dba` animation database holds offsets that reach past its computed span into a shared
    /// keyframe pool.
    #[must_use]
    pub fn chunk_data_to_end(&self, index: usize) -> Option<&[u8]> {
        let chunk = self.chunks.get(index)?;
        let start = usize::try_from(chunk.offset).ok()?;
        self.bytes.as_ref().get(start..)
    }

    /// Returns the chunks of `chunk_type` with their index in the table, in file order.
    pub fn chunks_of(&self, chunk_type: ChunkType) -> impl Iterator<Item = (usize, &Chunk)> {
        self.chunks
            .iter()
            .enumerate()
            .filter(move |(_, chunk)| chunk.kind == chunk_type)
    }

    /// Returns the first chunk with the `CrCh` chunk ID `id` and its index in the table; `#ivo`
    /// chunks have no ID and are never found.
    #[must_use]
    pub fn chunk_by_id(&self, id: u32) -> Option<(usize, &Chunk)> {
        self.chunks
            .iter()
            .enumerate()
            .find(|(_, chunk)| chunk.id == Some(id))
    }
}

/// Byte intervals for the stored words of a validated chunk table row.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChunkTableRow {
    /// The chunk type word.
    pub kind: Range<u64>,
    /// The layout version, including the byte-order flag in CrCh.
    pub version: Range<u64>,
    /// The chunk ID word when stored.
    pub id: Option<Range<u64>>,
    /// The payload offset word.
    pub offset: Range<u64>,
    /// The payload size word when stored.
    pub size: Option<Range<u64>>,
}

impl ChunkTableRow {
    pub(crate) fn at(start: u64, container: Container) -> Self {
        let at = |from: u64, to: u64| start + from..start + to;
        match container {
            Container::Ivo => Self {
                kind: at(0, 4),
                version: at(4, 8),
                id: None,
                offset: at(8, 16),
                size: None,
            },
            Container::CrCh => Self {
                kind: at(0, 2),
                version: at(2, 4),
                id: Some(at(4, 8)),
                offset: at(12, 16),
                size: Some(at(8, 12)),
            },
        }
    }
}

pub(crate) struct ChunkHeader {
    pub(crate) container: Container,
    pub(crate) table: Range<u64>,
    pub(crate) count: u32,
    len: u64,
}

impl ChunkHeader {
    pub(crate) fn read(bytes: &[u8], len: u64) -> Result<Self, ChunkError> {
        let mut reader = ByteReader::new(bytes);
        let signature = reader.read_bytes(4)?;
        let container =
            Container::from_signature(signature).ok_or_else(|| ChunkError::UnknownSignature {
                signature: <[u8; 4]>::try_from(signature).unwrap_or_default(),
            })?;
        let version = reader.read_u32()?;
        if version != container.version() {
            return Err(ChunkError::UnsupportedVersion { container, version });
        }
        let count = reader.read_u32()?;
        let start = u64::from(reader.read_u32()?);
        let end = start + u64::from(count) * TABLE_ENTRY_LEN as u64;
        let header = Self {
            container,
            table: start..end,
            count,
            len,
        };
        if start < HEADER_LEN as u64 || end > len {
            return Err(header.table_error());
        }
        Ok(header)
    }

    pub(crate) const fn table_error(&self) -> ChunkError {
        ChunkError::TableOutOfBounds {
            offset: self.table.start,
            count: self.count,
            len: self.len,
        }
    }
}

pub(crate) fn read_table<E: From<ChunkError>>(
    header: &ChunkHeader,
    bytes: &[u8],
    mut checkpoint: impl FnMut() -> Result<(), E>,
) -> Result<Vec<Chunk>, E> {
    let layout = Layout {
        len: header.len,
        table: header.table.clone(),
    };
    let table = ByteReader::with_base(bytes, header.table.start);
    match header.container {
        Container::Ivo => read_ivo(&layout, table, &mut checkpoint),
        Container::CrCh => read_crch(&layout, table, &mut checkpoint),
    }
}

fn read_crch<E: From<ChunkError>>(
    layout: &Layout,
    mut table: ByteReader<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<Chunk>, E> {
    let count = table.remaining() / TABLE_ENTRY_LEN;
    let mut chunks = Vec::new();
    chunks
        .try_reserve_exact(count)
        .map_err(|_| ChunkError::Allocation)?;
    for index in 0..count {
        checkpoint()?;
        let type_id = table.read_u16().map_err(ChunkError::from)?;
        let raw_version = table.read_u16().map_err(ChunkError::from)?;
        let id = table.read_u32().map_err(ChunkError::from)?;
        let size = u64::from(table.read_u32().map_err(ChunkError::from)?);
        let offset = u64::from(table.read_u32().map_err(ChunkError::from)?);
        layout.check(index, offset, size, Span::Exact)?;

        let byte_order = if raw_version & CRCH_BIG_ENDIAN_FLAG == 0 {
            ByteOrder::Little
        } else {
            ByteOrder::Big
        };
        chunks.push(Chunk {
            kind: ChunkType::new(Container::CrCh, u32::from(type_id)),
            version: u32::from(raw_version & !CRCH_BIG_ENDIAN_FLAG),
            id: Some(id),
            offset,
            size,
            byte_order,
        });
    }
    Ok(chunks)
}

fn read_ivo<E: From<ChunkError>>(
    layout: &Layout,
    mut table: ByteReader<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<Chunk>, E> {
    let len = layout.len;
    let count = table.remaining() / TABLE_ENTRY_LEN;
    let mut chunks = Vec::new();
    chunks
        .try_reserve_exact(count)
        .map_err(|_| ChunkError::Allocation)?;
    for index in 0..count {
        checkpoint()?;
        let type_id = table.read_u32().map_err(ChunkError::from)?;
        let version = table.read_u32().map_err(ChunkError::from)?;
        let offset = table.read_u64().map_err(ChunkError::from)?;
        layout.check(index, offset, 0, Span::Start)?;

        chunks.push(Chunk {
            kind: ChunkType::new(Container::Ivo, type_id),
            version,
            id: None,
            offset,
            size: 0,
            byte_order: ByteOrder::Little,
        });
    }
    let mut order = Vec::new();
    order
        .try_reserve_exact(count)
        .map_err(|_| ChunkError::Allocation)?;
    for (index, chunk) in chunks.iter().enumerate() {
        checkpoint()?;
        order.push((chunk.offset, index));
    }
    checkpoint()?;
    order.sort_unstable();
    checkpoint()?;
    let table_start = layout.table.start;
    let mut next = len;
    let mut group = len;
    for &(offset, index) in order.iter().rev() {
        checkpoint()?;
        if offset != group {
            next = group;
            group = offset;
        }
        let end = if table_start >= offset && table_start < next {
            table_start
        } else {
            next
        };
        if let Some(chunk) = chunks.get_mut(index) {
            chunk.size = end.saturating_sub(offset);
        }
    }
    Ok(chunks)
}

#[derive(Clone, Copy)]
enum Span {
    Start,
    Exact,
}

struct Layout {
    len: u64,
    table: Range<u64>,
}

impl Layout {
    fn check(&self, index: usize, offset: u64, size: u64, span: Span) -> Result<(), ChunkError> {
        let len = self.len;
        let end = offset.checked_add(size).filter(|&end| end <= len).ok_or(
            ChunkError::ChunkOutOfBounds {
                index,
                offset,
                size,
                len,
            },
        )?;
        let overlaps = match span {
            Span::Start => {
                offset < HEADER_LEN as u64 || (self.table.start < offset && offset < self.table.end)
            }
            Span::Exact => {
                size > 0
                    && (offset < HEADER_LEN as u64
                        || (offset < self.table.end && self.table.start < end))
            }
        };
        if overlaps {
            return Err(ChunkError::ChunkInHeaderOrTable { index, offset });
        }
        Ok(())
    }
}
