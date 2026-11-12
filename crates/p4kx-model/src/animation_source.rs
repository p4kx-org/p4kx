// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{ChunkFile, ChunkType, ivo};
use p4kx_foundation::ParseError;

use crate::animation_read::{
    ANIM_INFO_VERSION, Block, CAF_DATA_VERSION, CAF_SIGNATURE, RECORD_LEN, animation_chunk,
    controllers, corrupt, read_block, read_record,
};
use crate::{AnimationController, AnimationError, CorruptAnimation};

/// The interpreted timing fields of an animation metadata record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AnimationTiming {
    /// The source frame rate.
    pub frames_per_second: u16,
    /// The declared last frame.
    pub end_frame: u32,
    /// Whether the source declares additive keys.
    pub additive: bool,
}

/// A DBA clip name and its position in the source file.
#[derive(Debug, Clone, Copy)]
pub struct DbaAnimationName<'a> {
    at: u64,
    bytes: &'a [u8],
}

impl<'a> DbaAnimationName<'a> {
    pub(crate) const fn of(at: u64, bytes: &'a [u8]) -> Self {
        Self { at, bytes }
    }

    /// Returns the name bytes without the NUL terminator.
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Returns the end-exclusive interval of the name and its NUL terminator.
    #[must_use]
    pub const fn range(&self) -> Range<u64> {
        self.at..self.at + self.bytes.len() as u64 + 1
    }
}

/// The original 48-byte animation metadata and its enclosing chunk version.
#[derive(Debug, Clone, Copy)]
pub struct AnimationMetadata<'a> {
    chunk_index: usize,
    at: u64,
    version: u32,
    bytes: &'a [u8; RECORD_LEN],
}

impl<'a> AnimationMetadata<'a> {
    pub(crate) const fn of(
        chunk_index: usize,
        at: u64,
        version: u32,
        bytes: &'a [u8; RECORD_LEN],
    ) -> Self {
        Self {
            chunk_index,
            at,
            version,
            bytes,
        }
    }

    /// Returns the original metadata bytes.
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8; RECORD_LEN] {
        self.bytes
    }

    /// Returns the end-exclusive metadata interval in the source file.
    #[must_use]
    pub const fn range(&self) -> Range<u64> {
        self.at..self.at + RECORD_LEN as u64
    }

    /// Returns the original chunk table ordinal.
    #[must_use]
    pub const fn chunk_index(&self) -> usize {
        self.chunk_index
    }

    /// Returns the metadata chunk version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns all original flag bits.
    #[must_use]
    pub const fn flags(&self) -> u32 {
        let &[a, b, c, d, ..] = self.bytes;
        u32::from_le_bytes([a, b, c, d])
    }

    /// Reads the frame rate, end frame and additive flag.
    ///
    /// # Errors
    ///
    /// Returns an error if the record declares a zero frame rate.
    pub fn timing(&self) -> Result<AnimationTiming, CorruptAnimation> {
        read_record(self.bytes)
    }
}

/// The original metadata and controller table of a CAF or DBA clip.
#[derive(Debug, Clone, Copy)]
pub struct AnimationSource<'a> {
    ordinal: usize,
    metadata: AnimationMetadata<'a>,
    database_name: Option<DbaAnimationName<'a>>,
    data: SourceChunk<'a>,
    block: Block,
}

impl<'a> AnimationSource<'a> {
    pub(crate) const fn of(
        ordinal: usize,
        metadata: AnimationMetadata<'a>,
        database_name: Option<DbaAnimationName<'a>>,
        data: SourceChunk<'a>,
        block: Block,
    ) -> Self {
        Self {
            ordinal,
            metadata,
            database_name,
            data,
            block,
        }
    }

    /// Reads a CAF clip's metadata and controller table.
    ///
    /// # Errors
    ///
    /// Returns an error for missing, unsupported or malformed chunk structure.
    pub fn read_caf<B: AsRef<[u8]>>(
        file_name: &str,
        file: &'a ChunkFile<B>,
    ) -> Result<Self, AnimationError> {
        let metadata = SourceChunk::read(
            file_name,
            file,
            ivo::ANIM_INFO,
            ANIM_INFO_VERSION,
            "AnimInfo",
        )?;
        let record = metadata.bytes.first_chunk().ok_or_else(|| {
            corrupt(
                file_name,
                "AnimInfo",
                ParseError::UnexpectedEnd {
                    offset: metadata.at,
                    needed: RECORD_LEN as u64,
                    available: metadata.bytes.len() as u64,
                }
                .into(),
            )
        })?;
        let data = SourceChunk::read(file_name, file, ivo::CAF_DATA, CAF_DATA_VERSION, "CafData")?;
        let block = read_block(data.bytes, 0, CAF_SIGNATURE)
            .map_err(|err| corrupt(file_name, "CafData", err))?;
        Ok(Self::of(
            0,
            AnimationMetadata::of(metadata.index, metadata.at, metadata.version, record),
            None,
            data,
            block,
        ))
    }

    /// Returns the zero-based clip ordinal in its source file.
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Returns the original 48-byte metadata record.
    #[must_use]
    pub const fn metadata(&self) -> AnimationMetadata<'a> {
        self.metadata
    }

    /// Returns a DBA clip's original name bytes without the terminating NUL.
    #[must_use]
    pub const fn database_name(&self) -> Option<DbaAnimationName<'a>> {
        self.database_name
    }

    /// Returns the original chunk table ordinal for the controller block.
    #[must_use]
    pub const fn data_chunk_index(&self) -> usize {
        self.data.index
    }

    /// Returns the version of the chunk with the controller block.
    #[must_use]
    pub const fn data_chunk_version(&self) -> u32 {
        self.data.version
    }

    /// Returns the end-exclusive interval of the block header and controller table.
    #[must_use]
    pub const fn controller_block_range(&self) -> Range<u64> {
        self.data.at + self.block.at as u64..self.data.at + self.block.end as u64
    }

    /// Visits the controller entries in authored order, including entries without keys.
    ///
    /// # Errors
    ///
    /// Returns an error if the controller table cannot be read.
    pub fn controllers(
        &self,
    ) -> Result<impl Iterator<Item = AnimationController<'a>> + use<'a>, CorruptAnimation> {
        controllers(self.data.to_end, &self.block, self.data.at)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceChunk<'a> {
    pub(crate) index: usize,
    pub(crate) at: u64,
    pub(crate) version: u32,
    pub(crate) bytes: &'a [u8],
    to_end: &'a [u8],
}

impl<'a> SourceChunk<'a> {
    pub(crate) fn read<B: AsRef<[u8]>>(
        file_name: &str,
        file: &'a ChunkFile<B>,
        kind: ChunkType,
        version: u32,
        name: &'static str,
    ) -> Result<Self, AnimationError> {
        let (index, chunk) = animation_chunk(file_name, file, kind, version, name)?;
        let missing = || AnimationError::MissingChunk {
            file: file_name.to_owned(),
            chunk: name,
        };
        Ok(Self {
            index,
            at: chunk.offset(),
            version,
            bytes: file.chunk_data(index).ok_or_else(missing)?,
            to_end: file.chunk_data_to_end(index).ok_or_else(missing)?,
        })
    }
}
