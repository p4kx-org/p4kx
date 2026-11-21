// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::sync::Arc;

use p4kx_chunks::{ChunkFile, ChunkTableRow};

use crate::ModelChunkWords;
use crate::model_chunk_words::Words;

/// Original chunk table coordinates of one admitted animation chunk.
#[derive(Debug, Clone)]
pub struct AnimationChunkCoordinates {
    ordinal: usize,
    row: Option<ChunkTableRow>,
    prefix: Option<ModelChunkWords>,
}

impl AnimationChunkCoordinates {
    pub(crate) fn of<B: AsRef<[u8]>>(file: &ChunkFile<B>, ordinal: usize) -> Self {
        Self {
            ordinal,
            row: file.table_row(ordinal),
            prefix: ModelChunkWords::read(file, ordinal + 1, Words::Type),
        }
    }

    /// Returns the original chunk table row ordinal.
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Returns the fields of the selected original chunk table row.
    #[must_use]
    pub const fn row(&self) -> Option<&ChunkTableRow> {
        self.row.as_ref()
    }

    /// Returns the examined type words through the selected chunk.
    #[must_use]
    pub const fn type_prefix(&self) -> Option<&ModelChunkWords> {
        self.prefix.as_ref()
    }
}

#[derive(Debug)]
pub(crate) struct ClipChunks {
    pub(crate) metadata: AnimationChunkCoordinates,
    pub(crate) data: AnimationChunkCoordinates,
}

/// The original file coordinates of the latest decoded frame of a clip.
#[derive(Debug, Clone)]
pub struct AnimationLastFrame {
    pub(crate) frame: u16,
    pub(crate) controller: usize,
    pub(crate) bytes: Range<u64>,
    pub(crate) descriptor: Range<u64>,
    pub(crate) kind: AnimationFrameChannel,
}

impl AnimationLastFrame {
    /// Returns the last frame's decoded index.
    #[must_use]
    pub const fn frame(&self) -> u16 {
        self.frame
    }

    /// Returns the original controller table ordinal.
    #[must_use]
    pub const fn controller(&self) -> usize {
        self.controller
    }

    /// Returns the original word or bitmap used for the last frame.
    #[must_use]
    pub fn bytes(&self) -> Range<u64> {
        self.bytes.clone()
    }

    /// Returns the original channel descriptor used to select the encoding.
    #[must_use]
    pub fn descriptor(&self) -> Range<u64> {
        self.descriptor.clone()
    }

    /// Returns the channel that supplied this frame.
    #[must_use]
    pub const fn channel(&self) -> AnimationFrameChannel {
        self.kind
    }
}

/// The authored animation channel that supplied a frame.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum AnimationFrameChannel {
    /// The rotation channel.
    Rotation,
    /// The translation channel.
    Translation,
}

/// Original coordinates retained by the successful animation decoder.
#[derive(Debug, Clone)]
pub struct AnimationClipCoordinates {
    pub(crate) chunks: Arc<ClipChunks>,
    pub(crate) timing: u64,
    pub(crate) name: Option<Range<u64>>,
    pub(crate) block: Range<u64>,
    pub(crate) ids: Range<u64>,
    pub(crate) last: Option<AnimationLastFrame>,
}

impl AnimationClipCoordinates {
    /// Returns the original metadata chunk selection.
    #[must_use]
    pub fn metadata_chunk(&self) -> &AnimationChunkCoordinates {
        &self.chunks.metadata
    }

    /// Returns the original controller chunk selection.
    #[must_use]
    pub fn data_chunk(&self) -> &AnimationChunkCoordinates {
        &self.chunks.data
    }

    /// Returns the original flag word.
    #[must_use]
    pub const fn flags(&self) -> Range<u64> {
        self.timing..self.timing + 4
    }

    /// Returns the original frame-rate word.
    #[must_use]
    pub const fn frame_rate(&self) -> Range<u64> {
        self.timing + 4..self.timing + 6
    }

    /// Returns the original last-frame word.
    #[must_use]
    pub const fn end_frame(&self) -> Range<u64> {
        self.timing + 16..self.timing + 20
    }

    /// Returns the original database path and terminator, if authored.
    #[must_use]
    pub fn database_path(&self) -> Option<Range<u64>> {
        self.name.clone()
    }

    /// Returns the controller block header and authored descriptor tables.
    #[must_use]
    pub fn controller_block(&self) -> Range<u64> {
        self.block.clone()
    }

    /// Returns the original contiguous controller ID table.
    #[must_use]
    pub fn controller_ids(&self) -> Range<u64> {
        self.ids.clone()
    }

    /// Returns the latest decoded frame and its original encoding.
    #[must_use]
    pub const fn last_frame(&self) -> Option<&AnimationLastFrame> {
        self.last.as_ref()
    }
}

/// Original selectors of a successfully read animation database.
#[derive(Debug, Clone)]
pub struct AnimationDatabaseSelection {
    pub(crate) chunks: Arc<ClipChunks>,
    pub(crate) count: Range<u64>,
}

impl AnimationDatabaseSelection {
    /// Returns the original metadata chunk selection.
    #[must_use]
    pub fn metadata_chunk(&self) -> &AnimationChunkCoordinates {
        &self.chunks.metadata
    }

    /// Returns the original controller chunk selection.
    #[must_use]
    pub fn data_chunk(&self) -> &AnimationChunkCoordinates {
        &self.chunks.data
    }

    /// Returns the authored clip-count word.
    #[must_use]
    pub fn count_range(&self) -> Range<u64> {
        self.count.clone()
    }
}
