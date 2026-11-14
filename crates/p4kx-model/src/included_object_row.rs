// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

const PLACEMENT_KIND: u32 = 1;
const KIND_16: u32 = 16;
pub(crate) const KIND_16_LEN: usize = 136;
const MODEL_INDEX_AT: usize = 60;
const TRANSFORM_AT: usize = 64;
const TRANSFORM_LEN: usize = 96;
const PALETTE_AT: usize = 172;

/// The length of a placement for a measured object-table layout word.
pub(crate) const fn placement_len(layout: u32) -> Option<usize> {
    match layout {
        8..=10 => Some(176),
        12 => Some(184),
        13 => Some(192),
        14 => Some(200),
        15 => Some(208),
        _ => None,
    }
}

/// A placement with its original selectors and binary64 transform words.
#[derive(Debug, Clone, Copy)]
pub struct IncludedPlacement<'chunk> {
    row_offset: u64,
    model_index: u16,
    transform: &'chunk [u8; TRANSFORM_LEN],
    palette_byte: u8,
}

impl<'chunk> IncludedPlacement<'chunk> {
    fn read(bytes: &'chunk [u8], row_offset: u64) -> Option<Self> {
        let model = bytes.get(MODEL_INDEX_AT..)?.first_chunk()?;
        let transform = bytes.get(TRANSFORM_AT..)?.first_chunk()?;
        Some(Self {
            row_offset,
            model_index: u16::from_le_bytes(*model),
            transform,
            palette_byte: *bytes.get(PALETTE_AT)?,
        })
    }

    /// Returns the original selector into the chunk's local model table.
    #[must_use]
    pub const fn model_index(self) -> u16 {
        self.model_index
    }

    /// Returns the chunk-relative byte range of the model selector.
    #[must_use]
    pub const fn model_index_range(self) -> Range<u64> {
        let start = self.row_offset + MODEL_INDEX_AT as u64;
        start..start + size_of::<u16>() as u64
    }

    /// Returns the original palette selector byte.
    #[must_use]
    pub const fn palette_byte(self) -> u8 {
        self.palette_byte
    }

    /// Returns the chunk-relative byte range of the palette selector.
    #[must_use]
    pub const fn palette_range(self) -> Range<u64> {
        let start = self.row_offset + PALETTE_AT as u64;
        start..start + size_of::<u8>() as u64
    }

    /// Returns the chunk-relative byte range of the row-major transform.
    #[must_use]
    pub const fn transform_range(self) -> Range<u64> {
        let start = self.row_offset + TRANSFORM_AT as u64;
        start..start + TRANSFORM_LEN as u64
    }

    /// Returns the exact little-endian binary64 words in the original row-major order.
    #[must_use]
    pub fn transform_words(self) -> [[u64; 4]; 3] {
        let mut words = [[0; 4]; 3];
        for (word, bytes) in words.iter_mut().flatten().zip(self.transform.as_chunks().0) {
            *word = u64::from_le_bytes(*bytes);
        }
        words
    }
}

/// The framing disposition of original object-table bytes.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum IncludedObjectKind<'chunk> {
    /// A complete placement in a measured layout.
    Placement(IncludedPlacement<'chunk>),
    /// A complete kind-16 record.
    Kind16,
    /// The table ended inside a record.
    Truncated {
        /// The record kind when its four bytes were readable.
        kind: Option<u32>,
    },
    /// The remaining bytes begin with a kind whose record length is unmeasured.
    UnmeasuredKind {
        /// The original record kind.
        kind: u32,
    },
    /// The layout word does not identify a measured layout.
    UnmeasuredLayout,
}

/// A positioned object-table record or its uninterpreted remainder.
#[derive(Debug, Clone, Copy)]
pub struct IncludedObjectRow<'chunk> {
    ordinal: usize,
    offset: u64,
    bytes: &'chunk [u8],
    kind: IncludedObjectKind<'chunk>,
}

impl<'chunk> IncludedObjectRow<'chunk> {
    /// Returns the row ordinal, counting every bounded record in object-table order.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }

    /// Returns the chunk-relative byte range of the record or remainder.
    #[must_use]
    pub const fn range(self) -> Range<u64> {
        self.offset..self.offset + self.bytes.len() as u64
    }

    /// Borrows the original record or remainder bytes.
    #[must_use]
    pub const fn bytes(self) -> &'chunk [u8] {
        self.bytes
    }

    /// Returns the framing disposition of these bytes.
    #[must_use]
    pub const fn kind(self) -> IncludedObjectKind<'chunk> {
        self.kind
    }
}

/// A borrowing iterator over an original `IncludedObjects` table.
#[derive(Debug)]
pub struct IncludedObjectRows<'chunk> {
    rest: &'chunk [u8],
    next_offset: u64,
    ordinal: usize,
    placement_len: Option<usize>,
}

impl<'chunk> IncludedObjectRows<'chunk> {
    pub(crate) const fn new(rest: &'chunk [u8], next_offset: u64, layout: u32) -> Self {
        Self {
            rest,
            next_offset,
            ordinal: 0,
            placement_len: placement_len(layout),
        }
    }

    fn bounded(&self) -> (&'chunk [u8], IncludedObjectKind<'chunk>) {
        let Some(placement_len) = self.placement_len else {
            return (self.rest, IncludedObjectKind::UnmeasuredLayout);
        };
        let Some(field) = self.rest.first_chunk() else {
            return (self.rest, IncludedObjectKind::Truncated { kind: None });
        };
        let kind = u32::from_le_bytes(*field);
        let length = match kind {
            PLACEMENT_KIND => placement_len,
            KIND_16 => KIND_16_LEN,
            _ => return (self.rest, IncludedObjectKind::UnmeasuredKind { kind }),
        };
        let truncated = IncludedObjectKind::Truncated { kind: Some(kind) };
        let Some(bytes) = self.rest.get(..length) else {
            return (self.rest, truncated);
        };
        match kind {
            KIND_16 => (bytes, IncludedObjectKind::Kind16),
            _ => IncludedPlacement::read(bytes, self.next_offset)
                .map_or((self.rest, truncated), |placement| {
                    (bytes, IncludedObjectKind::Placement(placement))
                }),
        }
    }
}

impl<'chunk> Iterator for IncludedObjectRows<'chunk> {
    type Item = IncludedObjectRow<'chunk>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            return None;
        }
        let (bytes, kind) = self.bounded();
        let row = IncludedObjectRow {
            ordinal: self.ordinal,
            offset: self.next_offset,
            bytes,
            kind,
        };
        self.rest = self.rest.get(bytes.len()..)?;
        self.next_offset += bytes.len() as u64;
        self.ordinal += 1;
        Some(row)
    }
}
