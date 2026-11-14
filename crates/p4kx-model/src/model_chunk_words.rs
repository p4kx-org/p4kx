// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::ChunkFile;

#[derive(Clone, Copy)]
pub(crate) enum Words {
    Type,
    TypeAndVersion,
}

/// A sequence of examined words in the original chunk table.
#[derive(Debug, Clone)]
pub struct ModelChunkWords {
    first: Range<u64>,
    stride: u64,
    count: usize,
}

impl ModelChunkWords {
    pub(crate) fn read<B: AsRef<[u8]>>(
        file: &ChunkFile<B>,
        count: usize,
        layout: Words,
    ) -> Option<Self> {
        let table = file.table_range();
        let stride = (table.end - table.start).checked_div(file.chunks().len() as u64)?;
        let first = file.table_row(0)?;
        let first = match layout {
            Words::Type => first.kind,
            Words::TypeAndVersion => first.kind.start..first.version.end,
        };
        Some(Self {
            first,
            stride,
            count,
        })
    }

    /// Returns the half-open range of the first examined words.
    #[must_use]
    pub fn first_word(&self) -> Range<u64> {
        self.first.clone()
    }

    /// Returns the byte stride between examined words.
    #[must_use]
    pub const fn stride(&self) -> u64 {
        self.stride
    }

    /// Returns the number of examined table rows.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }
}
