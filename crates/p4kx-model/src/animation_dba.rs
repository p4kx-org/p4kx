// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_foundation::ByteReader;
use rayon::prelude::*;

use crate::animation_read::{
    Block, MAX_ANIMATION_KEYS, MAX_DATABASE_KEYS, RECORD_LEN, build, check_budget, chunk_bytes,
    corrupt, declared_keys, decode_tracks, listed_tracks, read_block, read_record, stem,
};
use crate::reader::to_usize;
use crate::{
    Animation, AnimationError, AnimationHeader, CorruptAnimation, PositionedAnimation,
    PositionedDba,
};

pub(crate) const DBA_DATA_VERSION: u32 = 0x900;
pub(crate) const DBA_METADATA_VERSION: u32 = 0x902;
pub(crate) const PREFIX_LEN: usize = 4;
pub(crate) const METADATA_HEADER_LEN: usize = 8;
pub(crate) const DBA_SIGNATURE: [u8; 4] = *b"#dba";
/// An animation of a `.dba` database that could not be read, with the reason; the other
/// animations of the database are unaffected.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SkippedAnimation {
    /// The animation's name.
    pub name: String,
    /// The path the database lists it under.
    pub path: String,
    /// Why it could not be read.
    pub reason: CorruptAnimation,
}

/// The animations of a `.dba` animation database.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Dba {
    /// The animations that were read, in the database's order.
    pub animations: Vec<Animation>,
    /// The animations that could not be read.
    pub skipped: Vec<SkippedAnimation>,
}

/// The headers of the animations of a `.dba` animation database.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DbaHeaders {
    /// The animations whose headers were read, in the database's order.
    pub animations: Vec<AnimationHeader>,
    /// The animations whose record or block header is unusable.
    pub skipped: Vec<SkippedAnimation>,
}

impl DbaHeaders {
    /// Reads the headers of the animations of the `.dba` file `file` named `file_name` without
    /// decoding a key; see [`Dba::read`] for the layout and the errors.
    ///
    /// # Errors
    ///
    /// As [`Dba::read`], except for errors in the keys themselves.
    pub fn read<B: AsRef<[u8]>>(
        file_name: &str,
        file: &ChunkFile<B>,
    ) -> Result<Self, AnimationError> {
        let metadata = chunk_bytes(
            file_name,
            file,
            ivo::DBA_METADATA,
            DBA_METADATA_VERSION,
            "DbaMetadata",
        )?;
        let data = chunk_bytes(file_name, file, ivo::DBA_DATA, DBA_DATA_VERSION, "DbaData")?;
        read_headers(metadata, data).map_err(|(chunk, source)| corrupt(file_name, chunk, source))
    }
}

impl Dba {
    /// Reads the animations of the `.dba` file `file` named `file_name`: the blocks of
    /// controllers of its `DbaData` chunk and, from its `DbaMetadata` chunk, the name, frame rate and
    /// end frame of each.
    ///
    /// An animation that cannot be read is left out of [`Self::animations`] and listed in
    /// [`Self::skipped`].
    ///
    /// # Errors
    ///
    /// Returns [`AnimationError::UnsupportedContainer`] for a `CrCh` file,
    /// [`AnimationError::MissingChunk`] and [`AnimationError::UnsupportedVersion`] when a chunk is
    /// absent or has a layout this reader does not know, and [`AnimationError::Corrupt`] when
    /// the metadata or a block header is truncated or malformed or the database declares more
    /// keys than [`MAX_DATABASE_KEYS`].
    pub fn read<B: AsRef<[u8]>>(
        file_name: &str,
        file: &ChunkFile<B>,
    ) -> Result<Self, AnimationError> {
        let decoded = read_projected(file_name, file, |animation, _, _| animation, Capture::Off)?;
        Ok(Self {
            animations: decoded.animations,
            skipped: decoded.skipped,
        })
    }

    /// Reads a DBA and retains the original position of each decoded clip and track.
    ///
    /// # Errors
    /// As [`Self::read`].
    pub fn read_with_positions<B: AsRef<[u8]>>(
        file_name: &str,
        file: &ChunkFile<B>,
    ) -> Result<PositionedDba, AnimationError> {
        let decoded = read_projected(
            file_name,
            file,
            PositionedAnimation::new,
            Capture::Positions,
        )?;
        Ok(PositionedDba {
            animations: decoded.animations,
            skipped: decoded.skipped,
        })
    }
}

fn read_projected<B: AsRef<[u8]>, T>(
    file_name: &str,
    file: &ChunkFile<B>,
    project: impl Fn(Animation, usize, Vec<usize>) -> T,
    capture: Capture,
) -> Result<DecodedDatabase<T>, AnimationError> {
    let metadata = chunk_bytes(
        file_name,
        file,
        ivo::DBA_METADATA,
        DBA_METADATA_VERSION,
        "DbaMetadata",
    )?;
    let data = chunk_bytes(file_name, file, ivo::DBA_DATA, DBA_DATA_VERSION, "DbaData")?;
    read_database(metadata, data, project, capture)
        .map_err(|(chunk, source)| corrupt(file_name, chunk, source))
}

type ChunkFailure = (&'static str, CorruptAnimation);

struct Layout<'a> {
    records: &'a [u8],
    paths: Vec<String>,
    blocks: Vec<Block>,
}

#[derive(Clone, Copy)]
enum Capture {
    Off,
    Positions,
}

struct DecodedDatabase<T> {
    animations: Vec<T>,
    skipped: Vec<SkippedAnimation>,
}

fn read_database<T>(
    metadata: &[u8],
    data: &[u8],
    project: impl Fn(Animation, usize, Vec<usize>) -> T,
    capture: Capture,
) -> Result<DecodedDatabase<T>, ChunkFailure> {
    let Layout {
        records,
        paths,
        blocks,
    } = layout(metadata, data)?;
    let count = blocks.len();
    let decoded: Vec<_> = blocks
        .par_iter()
        .zip(records.par_chunks_exact(RECORD_LEN))
        .map(|(block, record)| {
            let record = read_record(record)?;
            check_budget(declared_keys(data, block)?, MAX_ANIMATION_KEYS)?;
            let mut controllers = Vec::new();
            let tracks = decode_tracks(data, block, |ordinal| {
                if matches!(capture, Capture::Positions) {
                    controllers.push(ordinal);
                }
            })?;
            Ok::<_, CorruptAnimation>((record, tracks, controllers))
        })
        .collect();
    let mut dba = DecodedDatabase {
        animations: Vec::with_capacity(count),
        skipped: Vec::new(),
    };
    for (ordinal, (path, result)) in paths.into_iter().zip(decoded).enumerate() {
        let name = stem(&path).to_owned();
        match result {
            Ok((record, tracks, controllers)) => {
                let animation = build(name, path, record, tracks);
                dba.animations
                    .push(project(animation, ordinal, controllers));
            }
            Err(reason) => dba.skipped.push(SkippedAnimation { name, path, reason }),
        }
    }
    Ok(dba)
}

fn read_headers(metadata: &[u8], data: &[u8]) -> Result<DbaHeaders, ChunkFailure> {
    let Layout {
        records,
        paths,
        blocks,
    } = layout(metadata, data)?;
    let mut headers = DbaHeaders {
        animations: Vec::with_capacity(blocks.len()),
        skipped: Vec::new(),
    };
    for ((path, block), record) in paths
        .into_iter()
        .zip(&blocks)
        .zip(records.as_chunks::<RECORD_LEN>().0)
    {
        let name = stem(&path).to_owned();
        let read = read_record(record).and_then(|record| {
            check_budget(declared_keys(data, block)?, MAX_ANIMATION_KEYS)?;
            let (ids, keys) = listed_tracks(data, block)?;
            let mut header = AnimationHeader::of(name.clone(), path.clone(), record);
            header.controller_ids = ids;
            header.keys = keys;
            Ok(header)
        });
        match read {
            Ok(header) => headers.animations.push(header),
            Err(reason) => headers
                .skipped
                .push(SkippedAnimation { name, path, reason }),
        }
    }
    Ok(headers)
}

fn layout<'a>(metadata: &'a [u8], data: &[u8]) -> Result<Layout<'a>, ChunkFailure> {
    let in_metadata = |source| ("DbaMetadata", source);
    let in_data = |source| ("DbaData", source);
    let (records, name_table) = metadata_parts(metadata).map_err(in_metadata)?;
    let count = records.len() / RECORD_LEN;
    let paths = names(name_table, count).map_err(in_metadata)?;

    let mut blocks = Vec::with_capacity(count);
    let mut position = PREFIX_LEN;
    let mut keys = 0_u64;
    for _ in 0..count {
        let block = read_block(data, position, DBA_SIGNATURE).map_err(in_data)?;
        keys = keys.saturating_add(declared_keys(data, &block).map_err(in_data)?);
        check_budget(keys, MAX_DATABASE_KEYS).map_err(in_data)?;
        position = block.end;
        blocks.push(block);
    }
    Ok(Layout {
        records,
        paths,
        blocks,
    })
}

pub(crate) fn metadata_parts(metadata: &[u8]) -> Result<(&[u8], &[u8]), CorruptAnimation> {
    let mut reader = ByteReader::new(metadata);
    let count = to_usize(reader.read_u32()?);
    reader.skip(METADATA_HEADER_LEN - 4)?;
    let records = reader.read_bytes(count.saturating_mul(RECORD_LEN))?;
    let names = reader.read_bytes(reader.remaining())?;
    Ok((records, names))
}

pub(crate) fn next_name(table: &[u8], count: usize) -> Result<(&[u8], &[u8]), CorruptAnimation> {
    let mut parts = table.splitn(2, |&byte| byte == 0);
    match (parts.next(), parts.next()) {
        (Some(name), Some(rest)) => Ok((name, rest)),
        _ => Err(CorruptAnimation::Names { animations: count }),
    }
}

fn names(mut table: &[u8], count: usize) -> Result<Vec<String>, CorruptAnimation> {
    (0..count)
        .map(|_| {
            let (name, rest) = next_name(table, count)?;
            table = rest;
            Ok(String::from_utf8_lossy(name).into_owned())
        })
        .collect()
}
