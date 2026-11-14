// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::sync::Arc;

use p4kx_chunks::{Chunk, ChunkFile, ChunkType, Container, ivo};
use p4kx_foundation::ByteReader;

use crate::animation_clip_coordinates::{
    AnimationChunkCoordinates, AnimationClipCoordinates, AnimationLastFrame, ClipChunks,
};
use crate::animation_controller::{CHANNEL_LEN, CONTROLLER_LEN};
use crate::animation_keys::bytes_at;
use crate::reader::to_usize;
use crate::{
    Animation, AnimationController, AnimationError, AnimationTiming, CorruptAnimation,
    PositionedAnimation, Track,
};

/// The most keys one animation may declare.
pub const MAX_ANIMATION_KEYS: u64 = 1 << 22;
/// The most keys all animations of one database may declare together.
pub const MAX_DATABASE_KEYS: u64 = 1 << 24;

pub(crate) const ANIM_INFO_VERSION: u32 = 0x901;
pub(crate) const CAF_DATA_VERSION: u32 = 0x900;
pub(crate) const RECORD_LEN: usize = 48;
const BLOCK_HEADER_LEN: usize = 12;
const ADDITIVE: u32 = 1;
pub(crate) const CAF_SIGNATURE: [u8; 4] = *b"#caf";

pub(crate) fn read_record(bytes: &[u8]) -> Result<AnimationTiming, CorruptAnimation> {
    let mut reader = ByteReader::new(bytes);
    let flags = reader.read_u32()?;
    let frames_per_second = reader.read_u16()?;
    reader.skip(2 + 2 + 2 + 4)?;
    let end_frame = reader.read_u32()?;
    if frames_per_second == 0 {
        return Err(CorruptAnimation::FrameRate);
    }
    Ok(AnimationTiming {
        additive: flags & ADDITIVE != 0,
        frames_per_second,
        end_frame,
    })
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Block {
    pub(crate) at: usize,
    pub(crate) count: usize,
    pub(crate) end: usize,
}

impl Block {
    pub(crate) const fn ids_at(self) -> usize {
        self.at + BLOCK_HEADER_LEN
    }
}

pub(crate) fn read_block(
    data: &[u8],
    at: usize,
    signature: [u8; 4],
) -> Result<Block, CorruptAnimation> {
    let head = bytes_at(data, at, BLOCK_HEADER_LEN)?;
    let mut reader = ByteReader::new(head);
    let found = <[u8; 4]>::try_from(reader.read_bytes(4)?).unwrap_or_default();
    if found != signature {
        return Err(CorruptAnimation::Signature {
            expected: signature,
            found,
        });
    }
    let count = usize::from(reader.read_u16()?);
    let tables = count * (4 + CONTROLLER_LEN);
    bytes_at(data, at + BLOCK_HEADER_LEN, tables)?;
    Ok(Block {
        at,
        count,
        end: at + BLOCK_HEADER_LEN + tables,
    })
}

pub(crate) fn declared_keys(data: &[u8], block: &Block) -> Result<u64, CorruptAnimation> {
    Ok(controllers(data, block, 0)?
        .map(|controller| u64::from(controller.declared_keys()))
        .sum())
}

/// Returns the IDs of the controllers with keys and the number of keys they declare.
pub(crate) fn listed_tracks(
    data: &[u8],
    block: &Block,
) -> Result<(Vec<u32>, usize), CorruptAnimation> {
    let mut ids = Vec::with_capacity(block.count);
    let mut keys = 0_usize;
    for controller in controllers(data, block, 0)? {
        let declared = controller.declared_keys();
        if declared > 0 {
            ids.push(controller.controller_id());
            keys = keys.saturating_add(to_usize(declared));
        }
    }
    Ok((ids, keys))
}

pub(crate) fn decode_tracks(
    chunk: AnimationChunk<'_>,
    block: &Block,
    positions: &mut Option<TrackPositions>,
) -> Result<Vec<Track>, CorruptAnimation> {
    let mut tracks = Vec::with_capacity(block.count);
    for controller in controllers(chunk.bytes, block, chunk.offset)? {
        let track = if let Some(positions) = positions.as_mut() {
            controller
                .decode_observing(Some(|last| positions.record(controller.ordinal(), last)))?
        } else {
            controller.decode()?
        };
        tracks.extend(track);
    }
    Ok(tracks)
}

#[derive(Default)]
pub(crate) struct TrackPositions {
    ordinals: Vec<usize>,
    last: Option<AnimationLastFrame>,
}

impl TrackPositions {
    fn record(&mut self, ordinal: usize, last: Option<AnimationLastFrame>) {
        self.ordinals.push(ordinal);
        if let Some(last) = last
            && self
                .last
                .as_ref()
                .is_none_or(|previous| last.frame() >= previous.frame())
        {
            self.last = Some(last);
        }
    }

    pub(crate) fn project(
        self,
        chunks: Arc<ClipChunks>,
        timing: u64,
        name: Option<Range<u64>>,
        data: AnimationChunk<'_>,
        block: &Block,
    ) -> (Vec<usize>, AnimationClipCoordinates) {
        let first_id = data.offset + block.ids_at() as u64;
        (
            self.ordinals,
            AnimationClipCoordinates {
                chunks,
                timing,
                name,
                block: data.offset + block.at as u64..data.offset + block.end as u64,
                ids: first_id..first_id + block.count as u64 * size_of::<u32>() as u64,
                last: self.last,
            },
        )
    }
}

pub(crate) fn controllers<'a>(
    data: &'a [u8],
    block: &Block,
    file_offset: u64,
) -> Result<impl Iterator<Item = AnimationController<'a>> + use<'a>, CorruptAnimation> {
    let ids_at = block.ids_at();
    let ids = bytes_at(data, ids_at, 4 * block.count)?.as_chunks::<4>().0;
    let table_at = ids_at + 4 * block.count;
    let table = bytes_at(data, table_at, CONTROLLER_LEN * block.count)?
        .as_chunks::<CHANNEL_LEN>()
        .0
        .as_chunks::<2>()
        .0;
    Ok(ids
        .iter()
        .zip(table)
        .enumerate()
        .map(move |(ordinal, (id, channels))| {
            AnimationController::of(
                data,
                file_offset,
                ordinal,
                ids_at + ordinal * 4,
                u32::from_le_bytes(*id),
                table_at + ordinal * CONTROLLER_LEN,
                channels,
            )
        }))
}

pub(crate) const fn check_budget(keys: u64, limit: u64) -> Result<(), CorruptAnimation> {
    if keys > limit {
        return Err(CorruptAnimation::TooManyKeys { keys, limit });
    }
    Ok(())
}

pub(crate) const fn build(
    name: String,
    path: String,
    record: AnimationTiming,
    tracks: Vec<Track>,
) -> Animation {
    Animation {
        name,
        path,
        frames_per_second: record.frames_per_second,
        end_frame: record.end_frame,
        additive: record.additive,
        tracks,
    }
}

pub(crate) fn stem(path: &str) -> &str {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file.rsplit_once('.').map_or(file, |(stem, _)| stem)
}

pub(crate) fn corrupt(file: &str, chunk: &'static str, source: CorruptAnimation) -> AnimationError {
    AnimationError::Corrupt {
        file: file.to_owned(),
        chunk,
        source,
    }
}

/// Returns the data of the first chunk of `chunk_type` from its start to the end of the file.
pub(crate) fn chunk_bytes<'f, B: AsRef<[u8]>>(
    file_name: &str,
    file: &'f ChunkFile<B>,
    chunk_type: ChunkType,
    version: u32,
    name: &'static str,
) -> Result<&'f [u8], AnimationError> {
    AnimationChunk::read(file_name, file, chunk_type, version, name).map(|chunk| chunk.bytes)
}

#[derive(Clone, Copy)]
pub(crate) struct AnimationChunk<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) offset: u64,
    index: usize,
}

impl<'a> AnimationChunk<'a> {
    pub(crate) fn read<B: AsRef<[u8]>>(
        file_name: &str,
        file: &'a ChunkFile<B>,
        chunk_type: ChunkType,
        version: u32,
        name: &'static str,
    ) -> Result<Self, AnimationError> {
        let (index, chunk) = animation_chunk(file_name, file, chunk_type, version, name)?;
        let bytes = file
            .chunk_data_to_end(index)
            .ok_or_else(|| AnimationError::MissingChunk {
                file: file_name.to_owned(),
                chunk: name,
            })?;
        Ok(Self {
            bytes,
            index,
            offset: chunk.offset(),
        })
    }

    pub(crate) fn observe<B: AsRef<[u8]>>(
        file: &ChunkFile<B>,
        metadata: Self,
        data: Self,
    ) -> Arc<ClipChunks> {
        Arc::new(ClipChunks {
            metadata: AnimationChunkCoordinates::of(file, metadata.index),
            data: AnimationChunkCoordinates::of(file, data.index),
        })
    }
}

pub(crate) fn animation_chunk<'a, B: AsRef<[u8]>>(
    file_name: &str,
    file: &'a ChunkFile<B>,
    chunk_type: ChunkType,
    version: u32,
    name: &'static str,
) -> Result<(usize, &'a Chunk), AnimationError> {
    if file.container() != Container::Ivo {
        return Err(AnimationError::UnsupportedContainer {
            file: file_name.to_owned(),
            container: file.container(),
        });
    }
    let (index, chunk) =
        file.chunks_of(chunk_type)
            .next()
            .ok_or_else(|| AnimationError::MissingChunk {
                file: file_name.to_owned(),
                chunk: name,
            })?;
    if chunk.version() != version {
        return Err(AnimationError::UnsupportedVersion {
            file: file_name.to_owned(),
            chunk: name,
            version: chunk.version(),
        });
    }
    Ok((index, chunk))
}

impl Animation {
    /// Reads the animation of the `.caf` file `file` named `file_name`: its `AnimInfo` chunk
    /// and its `CafData` chunk with one block of controllers. The animation is named after the
    /// file without folder and extension.
    ///
    /// # Errors
    ///
    /// Returns [`AnimationError::UnsupportedContainer`] for a `CrCh` file,
    /// [`AnimationError::MissingChunk`] and [`AnimationError::UnsupportedVersion`] when a chunk is
    /// absent or has a layout this reader does not know, and [`AnimationError::Corrupt`] when
    /// the data is truncated, malformed or in a key format this reader does not know.
    pub fn read_caf<B: AsRef<[u8]>>(
        file_name: &str,
        file: &ChunkFile<B>,
    ) -> Result<Self, AnimationError> {
        Self::read_caf_projected(file_name, file, None, |animation, _, _, _| animation)
    }

    /// Reads a CAF and retains each successful track's original controller ordinal.
    ///
    /// # Errors
    /// As [`Self::read_caf`].
    pub fn read_caf_with_positions<B: AsRef<[u8]>>(
        file_name: &str,
        file: &ChunkFile<B>,
    ) -> Result<PositionedAnimation, AnimationError> {
        Self::read_caf_projected(
            file_name,
            file,
            Some(TrackPositions::default()),
            PositionedAnimation::new,
        )
    }

    fn read_caf_projected<B: AsRef<[u8]>, T>(
        file_name: &str,
        file: &ChunkFile<B>,
        mut positions: Option<TrackPositions>,
        project: impl FnOnce(Self, usize, Vec<usize>, Option<AnimationClipCoordinates>) -> T,
    ) -> Result<T, AnimationError> {
        let info = AnimationChunk::read(
            file_name,
            file,
            ivo::ANIM_INFO,
            ANIM_INFO_VERSION,
            "AnimInfo",
        )?;
        let record = read_record(info.bytes.get(..RECORD_LEN).unwrap_or(info.bytes))
            .map_err(|source| corrupt(file_name, "AnimInfo", source))?;
        let data =
            AnimationChunk::read(file_name, file, ivo::CAF_DATA, CAF_DATA_VERSION, "CafData")?;
        let block = read_block(data.bytes, 0, CAF_SIGNATURE)
            .map_err(|source| corrupt(file_name, "CafData", source))?;
        let tracks = declared_keys(data.bytes, &block)
            .and_then(|keys| {
                check_budget(keys, MAX_ANIMATION_KEYS)?;
                decode_tracks(data, &block, &mut positions)
            })
            .map_err(|source| corrupt(file_name, "CafData", source))?;
        let animation = build(
            stem(file_name).to_owned(),
            file_name.to_owned(),
            record,
            tracks,
        );
        let (ordinals, coordinates) = positions.map_or_else(
            || (Vec::new(), None),
            |positions| {
                let chunks = AnimationChunk::observe(file, info, data);
                let (ordinals, coordinates) =
                    positions.project(chunks, info.offset, None, data, &block);
                (ordinals, Some(coordinates))
            },
        );
        Ok(project(animation, 0, ordinals, coordinates))
    }
}
