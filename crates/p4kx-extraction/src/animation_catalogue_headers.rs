// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::ops::Range;

use p4kx_chunks::{ChunkError, ChunkFile};
use p4kx_model::{
    AnimationChannel, AnimationError, AnimationMetadata, AnimationSource, AnimationTiming,
    CorruptAnimation, DbaAnimationName, DbaAnimationSources, DbaMetadataRecords,
};

use crate::animation_load::{Kind, kind};

/// The readable physical clip headers of a source CAF or DBA.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct AnimationSourceHeaders {
    /// Clip declarations in original physical order.
    pub clips: Vec<AnimationSourceHeader>,
    /// Structural diagnostic for an incomplete metadata or controller stream.
    pub partial: Option<AnimationHeaderFault>,
}

/// Failure to open original animation metadata.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AnimationHeaderError {
    /// The request does not name a CAF or DBA.
    #[error("the source is not a CAF or DBA")]
    UnsupportedPath,
    /// The input is not a readable chunk container.
    #[error(transparent)]
    Container(#[from] ChunkError),
    /// Original clip metadata cannot be located or decoded.
    #[error(transparent)]
    Metadata(#[from] AnimationError),
}

/// Why some original clip headers or controller declarations are unavailable.
#[derive(Debug)]
#[non_exhaustive]
pub enum AnimationHeaderFault {
    /// A CAF or DBA chunk or its record sequence could not be read.
    Structure(AnimationError),
    /// The controller declarations of one known clip could not be read.
    Controllers {
        /// Physical clip ordinal.
        clip: usize,
        /// Original controller-table error.
        reason: CorruptAnimation,
    },
}

/// One original clip declaration, independent of display name and controller availability.
#[derive(Debug)]
#[non_exhaustive]
pub struct AnimationSourceHeader {
    /// Original CAF or DBA clip ordinal.
    pub ordinal: usize,
    /// Original chunk table ordinal of the metadata record.
    pub metadata_chunk: usize,
    /// Layout version of the original metadata chunk.
    pub metadata_version: u32,
    /// Half-open interval of the metadata record in the decoded Entry.
    pub metadata_range: Range<u64>,
    /// Exact original fixed-width metadata record.
    pub metadata_bytes: [u8; 48],
    /// Original flags, including reserved bits.
    pub flags: u32,
    /// Interpreted timing or its source diagnostic.
    pub timing: Result<AnimationTiming, CorruptAnimation>,
    /// Original DBA path bytes and range, absent for the filename-derived CAF name.
    pub database_name: Option<AnimationHeaderName>,
    /// Controller header evidence when the original block is readable.
    pub controller_block: Option<AnimationControllerBlock>,
}

/// Exact stored DBA path and address.
#[derive(Debug)]
#[non_exhaustive]
pub struct AnimationHeaderName {
    /// Bytes of the original path without its trailing NUL.
    pub bytes: Vec<u8>,
    /// Half-open interval of the original path, including its NUL.
    pub range: Range<u64>,
}

/// One original controller block with no decoded key payload.
#[derive(Debug)]
#[non_exhaustive]
pub struct AnimationControllerBlock {
    /// Original controller chunk table ordinal.
    pub chunk: usize,
    /// Layout version of the original controller chunk.
    pub version: u32,
    /// Half-open interval of the block header and controller table.
    pub range: Range<u64>,
    /// Original controller rows in physical order.
    pub controllers: Vec<AnimationHeaderController>,
}

/// Controller identity and exact channel declarations.
#[derive(Debug)]
#[non_exhaustive]
pub struct AnimationHeaderController {
    /// Original physical row within the clip.
    pub ordinal: usize,
    /// Stored controller ID.
    pub id: u32,
    /// Original byte interval of the ID.
    pub id_range: Range<u64>,
    /// Original byte interval of the controller descriptor.
    pub table_range: Range<u64>,
    /// Stored rotation channel descriptor.
    pub rotation: AnimationChannel,
    /// Stored translation channel descriptor.
    pub translation: AnimationChannel,
}

enum HeaderRead<E> {
    Chunk(ChunkError),
    Interrupted(E),
}

impl<E> From<ChunkError> for HeaderRead<E> {
    fn from(error: ChunkError) -> Self {
        Self::Chunk(error)
    }
}

impl AnimationSourceHeaders {
    /// Reads clip metadata and controller declarations without decoding animation keys.
    ///
    /// # Errors
    /// Returns an error for an invalid container, unsupported file kind or unreadable metadata.
    pub fn read(path: &str, bytes: &[u8]) -> Result<Self, AnimationHeaderError> {
        match Self::read_with(path, bytes, &mut || Ok::<_, Infallible>(())) {
            Ok(result) => result,
            Err(never) => match never {},
        }
    }

    /// Reads original headers and polls before each clip and controller occurrence.
    ///
    /// # Errors
    /// The outer error propagates caller interruption; the inner error describes unreadable headers.
    pub fn read_with<E>(
        path: &str,
        bytes: &[u8],
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Result<Self, AnimationHeaderError>, E> {
        check()?;
        let Some(kind) = kind(path) else {
            return Ok(Err(AnimationHeaderError::UnsupportedPath));
        };
        let file = match ChunkFile::parse_with(bytes, || check().map_err(HeaderRead::Interrupted)) {
            Ok(file) => file,
            Err(HeaderRead::Chunk(error)) => return Ok(Err(error.into())),
            Err(HeaderRead::Interrupted(error)) => return Err(error),
        };
        check()?;
        let inventory = match kind {
            Kind::Caf => Self::caf(path, &file, check)?,
            Kind::Dba => Self::dba(path, &file, check)?,
        };
        Ok(inventory.map_err(AnimationHeaderError::from))
    }

    fn caf<E>(
        path: &str,
        file: &ChunkFile<&[u8]>,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Result<Self, AnimationError>, E> {
        let mut partial = None;
        let metadata = match AnimationMetadata::read_caf(path, file) {
            Ok(metadata) => metadata,
            Err(error) => return Ok(Err(error)),
        };
        let controller_block = match AnimationSource::read_caf(path, file) {
            Ok(source) => block(source, &mut partial, check)?,
            Err(error) => {
                partial = Some(AnimationHeaderFault::Structure(error));
                None
            }
        };
        Ok(Ok(Self {
            clips: vec![AnimationSourceHeader::new(
                0,
                metadata,
                None,
                controller_block,
            )],
            partial,
        }))
    }

    fn dba<E>(
        path: &str,
        file: &ChunkFile<&[u8]>,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Result<Self, AnimationError>, E> {
        let mut inventory = Self::default();
        let sources = match DbaAnimationSources::read(path, file) {
            Ok(sources) => Some(sources),
            Err(error) => {
                inventory.partial = Some(AnimationHeaderFault::Structure(error));
                None
            }
        };
        for source in sources.into_iter().flatten() {
            check()?;
            let source = match source {
                Ok(source) => source,
                Err(error) => {
                    inventory
                        .partial
                        .get_or_insert(AnimationHeaderFault::Structure(error));
                    break;
                }
            };
            let controller_block = block(source, &mut inventory.partial, check)?;
            inventory.clips.push(AnimationSourceHeader::new(
                source.ordinal(),
                source.metadata(),
                source.database_name(),
                controller_block,
            ));
        }
        if inventory.partial.is_some() {
            return inventory.metadata_tail(path, file, check);
        }
        Ok(Ok(inventory))
    }

    fn metadata_tail<E>(
        mut self,
        path: &str,
        file: &ChunkFile<&[u8]>,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Result<Self, AnimationError>, E> {
        let records = match DbaMetadataRecords::read(path, file) {
            Ok(records) => records,
            Err(error) => return Ok(Err(error)),
        };
        let known = self.clips.len();
        for (position, record) in records.enumerate() {
            check()?;
            if position < known {
                continue;
            }
            let record = match record {
                Ok(record) => record,
                Err(error) => {
                    self.partial
                        .get_or_insert(AnimationHeaderFault::Structure(error));
                    break;
                }
            };
            self.clips.push(AnimationSourceHeader::new(
                record.ordinal,
                record.metadata,
                Some(record.name),
                None,
            ));
        }
        Ok(Ok(self))
    }
}

impl AnimationSourceHeader {
    fn new(
        ordinal: usize,
        metadata: AnimationMetadata<'_>,
        name: Option<DbaAnimationName<'_>>,
        controller_block: Option<AnimationControllerBlock>,
    ) -> Self {
        Self {
            ordinal,
            metadata_chunk: metadata.chunk_index(),
            metadata_version: metadata.version(),
            metadata_range: metadata.range(),
            metadata_bytes: *metadata.bytes(),
            flags: metadata.flags(),
            timing: metadata.timing(),
            database_name: name.map(|name| AnimationHeaderName {
                bytes: name.bytes().to_owned(),
                range: name.range(),
            }),
            controller_block,
        }
    }
}

fn block<E>(
    source: AnimationSource<'_>,
    partial: &mut Option<AnimationHeaderFault>,
    check: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<AnimationControllerBlock>, E> {
    check()?;
    let controllers = match source.controllers() {
        Ok(controllers) => controllers,
        Err(error) => {
            partial.get_or_insert(AnimationHeaderFault::Controllers {
                clip: source.ordinal(),
                reason: error,
            });
            return Ok(None);
        }
    };
    let controllers = controllers
        .map(|controller| {
            check()?;
            Ok(AnimationHeaderController {
                ordinal: controller.ordinal(),
                id: controller.controller_id(),
                id_range: controller.id_range(),
                table_range: controller.table_range(),
                rotation: controller.rotation(),
                translation: controller.translation(),
            })
        })
        .collect::<Result<_, E>>()?;
    Ok(Some(AnimationControllerBlock {
        chunk: source.data_chunk_index(),
        version: source.data_chunk_version(),
        range: source.controller_block_range(),
        controllers,
    }))
}
