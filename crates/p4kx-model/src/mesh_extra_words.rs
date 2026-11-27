// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_foundation::{ByteReader, ParseError};

use crate::reader::to_usize;

const WORD_BYTES: usize = size_of::<u32>();

/// One raw word in an IVO mesh's table after the submesh descriptors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshExtraWord {
    value: u32,
    start: u64,
}

impl MeshExtraWord {
    /// Returns the stored little-endian word.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.value
    }

    /// Returns the word's half-open byte range in the decoded source file.
    #[must_use]
    pub const fn range(self) -> Range<u64> {
        self.start..self.start + WORD_BYTES as u64
    }

    pub(crate) const fn with_base(mut self, base: u64) -> Self {
        self.start += base;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MeshExtraWords<'input> {
    words: &'input [[u8; WORD_BYTES]],
    start: u64,
}

impl<'input> MeshExtraWords<'input> {
    pub(crate) fn read(reader: &mut ByteReader<'input>, count: u32) -> Result<Self, ParseError> {
        let start = reader.offset();
        let (words, _) = reader
            .read_bytes(to_usize(count).saturating_mul(WORD_BYTES))?
            .as_chunks();
        Ok(Self { words, start })
    }

    pub(crate) fn get(self, index: u16) -> Option<MeshExtraWord> {
        let value = u32::from_le_bytes(*self.words.get(usize::from(index))?);
        let start = self.start + u64::from(index) * WORD_BYTES as u64;
        Some(MeshExtraWord { value, start })
    }
}
