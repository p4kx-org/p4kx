// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{Chunk, ChunkFile, ivo};

use p4kx_foundation::{ByteReader, ParseError};

use crate::physics_blob::{self, PhysicsBlob};

const BLOB_COUNT_OFFSET: usize = 40;
const TABLE_OFFSET: usize = 80;
const SLOT_BYTES: usize = 8;
const WORD_BYTES: usize = size_of::<u32>();
const CHUNK_VERSION: u32 = 0x900;

/// Failure to select a supported original collision chunk.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StatObjPhysicsError {
    /// The requested slot is absent or holds a different chunk type.
    #[error("chunk {index} is not a StatObjPhysics chunk")]
    NotPhysics {
        /// Original chunk table ordinal.
        index: usize,
    },
    /// The original chunk version is outside the supported table layout.
    #[error("StatObjPhysics chunk {index} has unsupported table version {version:#x}")]
    UnsupportedVersion {
        /// Original chunk table ordinal.
        index: usize,
        /// Original table version.
        version: u32,
    },
    /// The original table does not give an available bounded chunk span.
    #[error("StatObjPhysics chunk {index} has no readable payload range")]
    ChunkRange {
        /// Original chunk table ordinal.
        index: usize,
    },
    /// The chunk's collision table is malformed.
    #[error(transparent)]
    Table(#[from] ParseError),
}

/// A positioned table of sequential collision blobs within an IVO `StatObjPhysics` chunk.
#[derive(Debug)]
pub struct StatObjPhysics<'source> {
    rows: &'source [u8],
    payload: &'source [u8],
    payload_start: u64,
}

impl<'source> StatObjPhysics<'source> {
    /// Reads the version-qualified original chunk at its physical table ordinal.
    ///
    /// # Errors
    ///
    /// Returns the selection error, unsupported version, or positioned binary read error.
    pub fn read_chunk<B: AsRef<[u8]>>(
        file: &'source ChunkFile<B>,
        index: usize,
    ) -> Result<Self, StatObjPhysicsError> {
        let chunk = file
            .chunks()
            .get(index)
            .ok_or(StatObjPhysicsError::NotPhysics { index })?;
        Self::require_layout(chunk, index)?;
        let bytes = file
            .chunk_data(index)
            .ok_or(StatObjPhysicsError::ChunkRange { index })?;
        Ok(Self::read(bytes)?)
    }

    /// Checks the original chunk type and measured table version.
    ///
    /// # Errors
    ///
    /// Returns a positioned selection or version error for unsupported chunks.
    pub fn require_layout(chunk: &Chunk, index: usize) -> Result<(), StatObjPhysicsError> {
        if chunk.chunk_type() != ivo::STAT_OBJ_PHYSICS {
            return Err(StatObjPhysicsError::NotPhysics { index });
        }
        if chunk.version() != CHUNK_VERSION {
            return Err(StatObjPhysicsError::UnsupportedVersion {
                index,
                version: chunk.version(),
            });
        }
        Ok(())
    }

    /// Returns the original slot count word's byte range within the chunk.
    #[must_use]
    pub const fn slot_count_range() -> Range<u64> {
        BLOB_COUNT_OFFSET as u64..(BLOB_COUNT_OFFSET + WORD_BYTES) as u64
    }

    /// Returns the original slot table's byte range within the chunk.
    #[must_use]
    pub const fn table_range(&self) -> Range<u64> {
        TABLE_OFFSET as u64..self.payload_start
    }

    /// Reads the original table and keeps the complete bounded payload for its slots.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the header or table is truncated or the table extent overflows.
    pub fn read(bytes: &'source [u8]) -> Result<Self, ParseError> {
        let mut reader = ByteReader::new(bytes);
        reader.skip(BLOB_COUNT_OFFSET)?;
        let count = reader.read_u32()? as usize;
        reader.skip(TABLE_OFFSET - BLOB_COUNT_OFFSET - WORD_BYTES)?;
        let table_bytes =
            count
                .checked_mul(SLOT_BYTES)
                .ok_or_else(|| ParseError::UnexpectedEnd {
                    offset: TABLE_OFFSET as u64,
                    needed: count as u64 * SLOT_BYTES as u64,
                    available: reader.remaining() as u64,
                })?;
        let rows = reader.read_bytes(table_bytes)?;
        let payload_start = reader.offset();
        let payload = reader.read_bytes(reader.remaining())?;
        Ok(Self {
            rows,
            payload,
            payload_start,
        })
    }

    /// Iterates the original slots in physical order, including duplicate keys and missing spans.
    pub fn slots(&self) -> impl ExactSizeIterator<Item = StatObjPhysicsSlot<'source>> {
        let mut cursor = self.payload_start;
        self.rows
            .as_chunks::<SLOT_BYTES>()
            .0
            .iter()
            .enumerate()
            .map(move |(ordinal, row)| {
                let &[
                    length_0,
                    length_1,
                    length_2,
                    length_3,
                    key_0,
                    key_1,
                    key_2,
                    key_3,
                ] = row;
                let length = u32::from_le_bytes([length_0, length_1, length_2, length_3]);
                let key = u32::from_le_bytes([key_0, key_1, key_2, key_3]);
                let declared_end = cursor + u64::from(length);
                let range = cursor..declared_end;
                cursor = declared_end;
                let start = usize::try_from(range.start - self.payload_start).ok();
                let end = usize::try_from(range.end - self.payload_start).ok();
                let payload = start
                    .zip(end)
                    .and_then(|(start, end)| self.payload.get(start..end));
                let offset = TABLE_OFFSET as u64 + ordinal as u64 * SLOT_BYTES as u64;
                StatObjPhysicsSlot {
                    ordinal,
                    offset,
                    key,
                    length,
                    range,
                    payload,
                }
            })
    }

    /// Returns the chunk-relative span of bytes left after the declared collision payloads.
    #[must_use]
    pub fn trailing_range(&self) -> Option<Range<u64>> {
        let end = self.payload_start + self.payload.len() as u64;
        let start = self
            .slots()
            .last()
            .map_or(self.payload_start, |slot| slot.range.end);
        (start <= end).then_some(start..end)
    }
}

/// One original `StatObjPhysics` slot with the declared blob extent and original packed key.
#[derive(Debug, Clone)]
pub struct StatObjPhysicsSlot<'source> {
    ordinal: usize,
    offset: u64,
    key: u32,
    length: u32,
    range: Range<u64>,
    payload: Option<&'source [u8]>,
}

impl<'source> StatObjPhysicsSlot<'source> {
    /// Returns the physical ordinal of this slot in the chunk.
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Returns the original unsigned packed key.
    #[must_use]
    pub const fn key(&self) -> u32 {
        self.key
    }

    /// Returns the original part number in the upper half of the key.
    #[must_use]
    pub const fn part(&self) -> u16 {
        let [_, _, low, high] = self.key.to_le_bytes();
        u16::from_le_bytes([low, high])
    }

    /// Returns the source NMC node index or `None` for the whole-object sentinel.
    #[must_use]
    pub const fn node(&self) -> Option<u16> {
        let [low, high, _, _] = self.key.to_le_bytes();
        let index = u16::from_le_bytes([low, high]);
        if index == u16::MAX { None } else { Some(index) }
    }

    /// Returns the original declared size word.
    #[must_use]
    pub const fn declared_length(&self) -> u32 {
        self.length
    }

    /// Returns the chunk-relative location of the declared size word.
    #[must_use]
    pub const fn length_range(&self) -> Range<u64> {
        self.offset..self.offset + WORD_BYTES as u64
    }

    /// Returns the chunk-relative location of the packed key.
    #[must_use]
    pub const fn key_range(&self) -> Range<u64> {
        self.offset + WORD_BYTES as u64..self.offset + SLOT_BYTES as u64
    }

    /// Returns the declared chunk-relative range, which can exceed the chunk.
    #[must_use]
    pub fn declared_range(&self) -> Range<u64> {
        self.range.clone()
    }

    /// Returns the original blob only when the complete declared range exists.
    #[must_use]
    pub const fn bytes(&self) -> Option<&'source [u8]> {
        self.payload
    }

    /// Decodes a checked blob when its marker and shape layout are supported.
    #[must_use]
    pub fn blob(&self) -> PhysicsBlob<'source> {
        self.payload
            .map_or(PhysicsBlob::MissingPayload, physics_blob::read)
    }
}
