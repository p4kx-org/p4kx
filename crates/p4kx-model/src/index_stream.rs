// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::CorruptChunk;
use crate::streams::{element, u16_at, u32_at};

/// The index list of a mesh, 16 or 32 bits per index.
#[derive(Debug, Clone, Copy)]
pub(crate) struct IndexStream<'a> {
    data: &'a [u8],
    wide: bool,
    start: u64,
}

impl IndexStream<'_> {
    pub(crate) const fn from_bytes(data: &[u8], wide: bool) -> IndexStream<'_> {
        Self::positioned(data, wide, 0)
    }

    pub(crate) const fn positioned(data: &[u8], wide: bool, start: u64) -> IndexStream<'_> {
        IndexStream { data, wide, start }
    }

    pub(crate) const fn word_start(&self, index: u32) -> u64 {
        self.start + index as u64 * self.width() as u64
    }

    pub(crate) const fn width(&self) -> u32 {
        if self.wide { 4 } else { 2 }
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
