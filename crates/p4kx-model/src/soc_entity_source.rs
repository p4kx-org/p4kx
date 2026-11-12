// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ByteOrder, Chunk, ChunkError, ChunkFile, Container};
use p4kx_cryxml::{SourceDocument, SourceElement, SourceReadError, SourceReadOptions};
use thiserror::Error;

use crate::Transform;
use crate::soc_entities::{self, ENTITIES};

/// A failure to open or interpret an authored SOC entity chunk.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SocEntitySourceError {
    /// The chunk table is invalid.
    #[error("cannot read the SOC chunk table")]
    Chunks(#[from] ChunkError),
    /// The source uses a chunk framing other than `CrCh`.
    #[error("SOC entity sources require a CrCh chunk table")]
    Container,
    /// The entity chunk uses big-endian framing.
    #[error("SOC entity chunk {chunk_index} is big-endian")]
    BigEndian {
        /// Original chunk table index.
        chunk_index: usize,
    },
    /// The original chunk payload range cannot be read.
    #[error("SOC entity chunk {chunk_index} has no readable payload range")]
    ChunkRange {
        /// Original chunk table index.
        chunk_index: usize,
    },
    /// The original XML body cannot be read.
    #[error("cannot read the XML of SOC entity chunk {chunk_index}")]
    Xml {
        /// Original chunk table index.
        chunk_index: usize,
        /// Original XML decoding error.
        #[source]
        source: SourceReadError,
    },
    /// The XML body does not have the expected entity-list root.
    #[error("SOC entity chunk {chunk_index} has no Entities root")]
    Root {
        /// Original chunk table index.
        chunk_index: usize,
    },
}

/// The authored entity chunks of one SOC file.
#[derive(Debug)]
pub struct SocEntityFile<'input> {
    chunks: ChunkFile<&'input [u8]>,
}

impl<'input> SocEntityFile<'input> {
    /// Opens the original chunk table.
    ///
    /// # Errors
    /// Returns an error when the `CrCh` chunk table is unavailable or unreadable.
    pub fn parse(bytes: &'input [u8]) -> Result<Self, SocEntitySourceError> {
        let chunks = ChunkFile::parse(bytes)?;
        if chunks.container() != Container::CrCh {
            return Err(SocEntitySourceError::Container);
        }
        Ok(Self { chunks })
    }

    /// Reads entity chunks in physical table order, reporting failures at their original index.
    pub fn entity_chunks(
        &self,
    ) -> impl Iterator<Item = Result<SocEntityChunk<'_>, SocEntitySourceError>> {
        self.chunks
            .chunks_of(ENTITIES)
            .map(|(index, header)| self.read_chunk(index, header))
    }

    fn read_chunk<'file>(
        &'file self,
        chunk_index: usize,
        header: &'file Chunk,
    ) -> Result<SocEntityChunk<'file>, SocEntitySourceError> {
        if header.byte_order() == ByteOrder::Big {
            return Err(SocEntitySourceError::BigEndian { chunk_index });
        }
        let bytes = self
            .chunks
            .chunk_data(chunk_index)
            .ok_or(SocEntitySourceError::ChunkRange { chunk_index })?;
        let document =
            SourceDocument::parse(bytes, SourceReadOptions::default()).map_err(|source| {
                SocEntitySourceError::Xml {
                    chunk_index,
                    source,
                }
            })?;
        if !document.root().name().eq_ignore_ascii_case("Entities") {
            return Err(SocEntitySourceError::Root { chunk_index });
        }
        Ok(SocEntityChunk {
            chunk_index,
            header,
            document,
        })
    }
}

/// An entity chunk and its original XML body.
#[derive(Debug)]
pub struct SocEntityChunk<'file> {
    chunk_index: usize,
    header: &'file Chunk,
    document: SourceDocument<'file>,
}

impl SocEntityChunk<'_> {
    /// Returns the zero-based index in the original chunk table.
    #[must_use]
    pub const fn chunk_index(&self) -> usize {
        self.chunk_index
    }

    /// Returns the original chunk header, including its payload offset in the SOC.
    #[must_use]
    pub const fn header(&self) -> &Chunk {
        self.header
    }

    /// Borrows the XML body whose source coordinates are relative to the chunk payload.
    #[must_use]
    pub const fn document(&self) -> &SourceDocument<'_> {
        &self.document
    }

    /// Visits every direct Entity in its original element slot.
    pub fn entities(&self) -> impl Iterator<Item = SocEntity<'_>> {
        self.document
            .root()
            .elements()
            .enumerate()
            .filter_map(|(ordinal, element)| {
                element
                    .name()
                    .eq_ignore_ascii_case("Entity")
                    .then_some(SocEntity { ordinal, element })
            })
    }
}

/// An original Entity element with its physical position in the owning XML body.
#[derive(Debug, Clone, Copy)]
pub struct SocEntity<'document> {
    ordinal: usize,
    element: SourceElement<'document>,
}

impl<'document> SocEntity<'document> {
    /// Returns the element's zero-based slot among the root's direct elements.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }

    /// Returns the original element and its attribute coordinates.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.element
    }

    /// Derives the container-local transform from unique `Pos`, `Rotate` and `Scale` attributes.
    #[must_use]
    pub fn local_transform(self) -> Option<Transform> {
        let mut position = None;
        let mut rotation = None;
        let mut scale = None;
        for attribute in self
            .element
            .attributes()
            .filter(|attribute| attribute.namespace().is_none())
        {
            let destination = match attribute.name() {
                "Pos" => &mut position,
                "Rotate" => &mut rotation,
                "Scale" => &mut scale,
                _ => continue,
            };
            if destination.replace(attribute.value()).is_some() {
                return None;
            }
        }
        soc_entities::transform_fields(position, rotation, scale)
    }
}
