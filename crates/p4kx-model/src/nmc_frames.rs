// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original `NodeMeshCombo` records used by physics projections.

use std::ops::Range;

use p4kx_chunks::{Chunk, ivo};
use p4kx_foundation::{ByteReader, ParseError};

use crate::nmc::{GEOMETRY_TYPE_AT, LOCAL_TRANSFORM_AT, NODE_LEN, PARENT_AT, SCALE_AT};

const COUNT_AT: usize = 4;
const RECORDS_AT: usize = 32;
const COMPANION_TRANSFORM_AT: usize = 32;
const STORED_ID_AT: usize = SCALE_AT + 3 * size_of::<u32>();

/// One decoded NMC row with its original 208-byte record.
#[derive(Debug, Clone, Copy)]
pub struct NmcFrame<'source> {
    ordinal: usize,
    bytes: &'source [u8; NODE_LEN],
}

/// Physical NMC row data independent of draw selection.
#[derive(Debug, Clone, Copy)]
pub struct NmcFrames<'source> {
    records: &'source [[u8; NODE_LEN]],
}

impl<'source> NmcFrames<'source> {
    /// Returns whether an original chunk uses the measured NMC layout.
    #[must_use]
    pub const fn supports(chunk: &Chunk) -> bool {
        matches!(
            (chunk.chunk_type(), chunk.version()),
            (ivo::NODE_MESH_COMBO, 0x900)
        )
    }

    /// Reads the variable-length record table without decoding geometry or strings.
    ///
    /// # Errors
    /// Returns the original parsing error for a missing count or incomplete record array.
    pub fn read(bytes: &'source [u8]) -> Result<Self, ParseError> {
        let mut reader = ByteReader::new(bytes);
        reader.skip(COUNT_AT)?;
        let count = reader.read_u32()? as usize;
        reader.skip(RECORDS_AT - 2 * size_of::<u32>())?;
        let (records, _) = reader
            .read_bytes(count.saturating_mul(NODE_LEN))?
            .as_chunks();
        Ok(Self { records })
    }

    /// Returns the number of original records.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns whether the physically authored node table is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns the positioned source row at the original ordinal.
    #[must_use]
    pub fn frame(&self, ordinal: usize) -> Option<NmcFrame<'source>> {
        Some(NmcFrame {
            ordinal,
            bytes: self.records.get(ordinal)?,
        })
    }

    /// Visits the actual rows, including helpers and geometry with no drawable mesh.
    pub fn frames(&self) -> impl ExactSizeIterator<Item = NmcFrame<'source>> {
        self.records
            .iter()
            .enumerate()
            .map(|(ordinal, bytes)| NmcFrame { ordinal, bytes })
    }

    /// Returns the chunk-relative count word used to admit the rows.
    #[must_use]
    pub const fn count_range() -> Range<u64> {
        COUNT_AT as u64..(COUNT_AT + size_of::<u32>()) as u64
    }
}

impl NmcFrame<'_> {
    /// Returns the physical node row index within the source NMC chunk.
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Reads the original packed parent word before verifying a parent chain.
    #[must_use]
    pub fn parent_word(&self) -> u16 {
        u16::from_le_bytes(bytes_at(self.bytes, PARENT_AT))
    }

    /// Reads the stored node identifier word, independently from the physical ordinal.
    #[must_use]
    pub fn stored_id(&self) -> u32 {
        word(self.bytes, STORED_ID_AT)
    }

    /// Reads the original geometry-type word.
    #[must_use]
    pub fn geometry_type(&self) -> u16 {
        u16::from_le_bytes(bytes_at(self.bytes, GEOMETRY_TYPE_AT))
    }

    /// Returns the original row-major words of the local affine field.
    #[must_use]
    pub fn local_matrix_bits(&self) -> [[u32; 4]; 3] {
        self.matrix_bits(LOCAL_TRANSFORM_AT)
    }

    /// Returns the original row-major words of the companion affine field.
    #[must_use]
    pub fn companion_matrix_bits(&self) -> [[u32; 4]; 3] {
        self.matrix_bits(COMPANION_TRANSFORM_AT)
    }

    /// Returns the three independently stored scale words.
    #[must_use]
    pub fn scale_bits(&self) -> [u32; 3] {
        words(self.bytes, SCALE_AT)
    }

    /// Returns the original chunk-relative position of a field within this row.
    #[must_use]
    pub const fn range(&self, field: NmcFrameField) -> Range<u64> {
        let (at, width) = field.layout();
        let start = RECORDS_AT as u64 + self.ordinal as u64 * NODE_LEN as u64 + at;
        start..start + width
    }

    fn matrix_bits(&self, at: usize) -> [[u32; 4]; 3] {
        std::array::from_fn(|row| words(self.bytes, at + row * 4 * size_of::<u32>()))
    }
}

/// A physical field in the measured NMC row layout.
#[derive(Debug, Clone, Copy)]
pub enum NmcFrameField {
    /// Stored node ID.
    StoredId,
    /// Stored parent word.
    Parent,
    /// Stored geometry type.
    GeometryType,
    /// Primary local matrix.
    LocalMatrix,
    /// Companion matrix.
    CompanionMatrix,
    /// Separate scale vector.
    Scale,
}

impl NmcFrameField {
    const fn layout(self) -> (u64, u64) {
        match self {
            Self::StoredId => (STORED_ID_AT as u64, 4),
            Self::Parent => (PARENT_AT as u64, 2),
            Self::GeometryType => (GEOMETRY_TYPE_AT as u64, 2),
            Self::LocalMatrix => (LOCAL_TRANSFORM_AT as u64, 48),
            Self::CompanionMatrix => (COMPANION_TRANSFORM_AT as u64, 48),
            Self::Scale => (SCALE_AT as u64, 12),
        }
    }
}

fn words<const N: usize>(bytes: &[u8; NODE_LEN], start: usize) -> [u32; N] {
    std::array::from_fn(|index| word(bytes, start + index * size_of::<u32>()))
}

fn word(bytes: &[u8; NODE_LEN], start: usize) -> u32 {
    u32::from_le_bytes(bytes_at(bytes, start))
}

fn bytes_at<const N: usize>(bytes: &[u8; NODE_LEN], start: usize) -> [u8; N] {
    let words = bytes
        .get(start..start + N)
        .and_then(|value| value.try_into().ok());
    #[expect(
        clippy::expect_used,
        reason = "the fixed field offsets lie inside one NMC record"
    )]
    words.expect("NMC field fits its fixed-width source record")
}

#[cfg(test)]
#[path = "nmc_frames_tests.rs"]
mod tests;
