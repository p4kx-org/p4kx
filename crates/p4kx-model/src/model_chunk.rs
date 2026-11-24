// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{ChunkFile, ChunkType};

use crate::{ChunkUsage, ModelChunkLayout, ModelFile};

/// One chunk of a model's files and how the model used it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ModelChunk {
    /// The file holding the chunk.
    pub file: ModelFile,
    /// The chunk's position in the file's chunk table.
    pub index: usize,
    /// The chunk's type.
    pub chunk_type: ChunkType,
    /// How the model used it.
    pub usage: ChunkUsage,
    /// Parsed physical table fields, including the container's byte-span convention.
    pub layout: ModelChunkLayout,
    pub(crate) type_id_word: Option<[u64; 2]>,
}

impl ModelChunk {
    /// Returns the original type ID word's range in the owning decoded file.
    #[must_use]
    pub fn type_id_range(&self) -> Option<Range<u64>> {
        self.type_id_word.map(|[start, end]| start..end)
    }
}

pub(crate) fn chunk_usage<B: AsRef<[u8]>>(
    file: &ChunkFile<B>,
    which: ModelFile,
    read: &[(ModelFile, usize)],
) -> Vec<ModelChunk> {
    file.chunks()
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            let chunk_type = chunk.chunk_type();
            let usage = if read.contains(&(which, index)) {
                ChunkUsage::Read
            } else if chunk_type.is_known() {
                ChunkUsage::Skipped
            } else {
                ChunkUsage::Unknown
            };
            ModelChunk {
                file: which,
                index,
                chunk_type,
                usage,
                layout: ModelChunkLayout::of(chunk),
                type_id_word: file
                    .table_row(index)
                    .map(|row| [row.kind.start, row.kind.end]),
            }
        })
        .collect()
}
