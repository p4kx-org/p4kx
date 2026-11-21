// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{ChunkFile, ChunkTableRow};

use crate::ModelChunkWords;
use crate::model_chunk_words::Words;

/// The original lookup performed by the selected node decoder.
#[derive(Debug)]
#[non_exhaustive]
pub enum ModelNodeSelection {
    /// The prefix searched for the first `NodeMeshCombo` chunk.
    NodeMeshCombo {
        /// The examined type words in the original chunk table.
        table: Option<ModelChunkWords>,
        /// The admitted chunk, when present.
        chunk: Option<ModelNodeMeshChunk>,
    },
    /// The original table examined for versioned `CrCh` node chunks.
    CrCh {
        /// The examined type and version words in the original chunk table.
        table: Option<ModelChunkWords>,
    },
}

/// Original header and chunk-table coordinates of the admitted node mesh chunk.
#[derive(Debug)]
pub struct ModelNodeMeshChunk {
    index: usize,
    start: u64,
    table: ChunkTableRow,
}

impl ModelNodeMeshChunk {
    /// Returns the original chunk ordinal.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the table row that selected the chunk's storage.
    #[must_use]
    pub const fn table_row(&self) -> &ChunkTableRow {
        &self.table
    }

    /// Returns the original count word.
    #[must_use]
    pub const fn count(&self) -> Range<u64> {
        self.start + 4..self.start + 8
    }

    /// Returns the list and name extents used to locate the name table.
    #[must_use]
    pub const fn name_layout(&self) -> Range<u64> {
        self.start + 12..self.start + 24
    }

    /// Returns the list and string extents used to locate the property table.
    #[must_use]
    pub const fn property_layout(&self) -> Range<u64> {
        self.start + 12..self.start + 28
    }
}

impl ModelNodeSelection {
    pub(crate) fn ivo<B: AsRef<[u8]>>(file: &ChunkFile<B>, selected: Option<(usize, u64)>) -> Self {
        let examined = selected.map_or_else(|| file.chunks().len(), |(index, _)| index + 1);
        let chunk = selected.and_then(|(index, start)| {
            Some(ModelNodeMeshChunk {
                index,
                start,
                table: file.table_row(index)?,
            })
        });
        Self::NodeMeshCombo {
            table: ModelChunkWords::read(file, examined, Words::Type),
            chunk,
        }
    }

    pub(crate) fn crch<B: AsRef<[u8]>>(file: &ChunkFile<B>) -> Self {
        Self::CrCh {
            table: ModelChunkWords::read(file, file.chunks().len(), Words::TypeAndVersion),
        }
    }
}
