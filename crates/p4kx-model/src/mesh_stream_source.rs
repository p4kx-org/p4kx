// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_foundation::{ByteReader, ParseError};

use crate::CorruptChunk;
use crate::streams::INDICES;

const WORD_BYTES: u64 = size_of::<u32>() as u64;
const ALIGNMENT: usize = 8;

/// Physical fields of one complete IVO mesh stream header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshStreamHeader {
    tag: u32,
    stride: u32,
    count: u32,
    start: u64,
}

impl MeshStreamHeader {
    /// Returns the authored stream tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        self.tag
    }

    /// Returns the authored number of bytes per element.
    #[must_use]
    pub const fn stride(self) -> u32 {
        self.stride
    }

    /// Returns the header count used to estimate this stream's extent.
    #[must_use]
    pub const fn count(self) -> u32 {
        self.count
    }

    /// Returns the stream tag's original byte interval.
    #[must_use]
    pub const fn tag_range(self) -> Range<u64> {
        self.start..self.start + WORD_BYTES
    }

    /// Returns the element-size word's original byte interval.
    #[must_use]
    pub const fn stride_range(self) -> Range<u64> {
        self.start + WORD_BYTES..self.start + 2 * WORD_BYTES
    }

    /// Returns whether the model reader recognises the stored tag and width.
    #[must_use]
    pub const fn is_recognized(self) -> bool {
        crate::streams::recognized(self.tag, self.stride)
    }

    /// Returns whether the extent uses the index count instead of the vertex count.
    #[must_use]
    pub const fn is_index_stream(self) -> bool {
        self.tag == INDICES
    }
}

/// One physical IVO stream with the bytes reachable under its count convention.
#[derive(Debug, Clone, Copy)]
pub struct MeshStreamSource<'input> {
    header: MeshStreamHeader,
    bytes: &'input [u8],
    padding_end: u64,
}

impl<'input> MeshStreamSource<'input> {
    /// Returns the original tag, element size and corresponding count.
    #[must_use]
    pub const fn header(self) -> MeshStreamHeader {
        self.header
    }

    /// Returns the bounded stream bytes, excluding its alignment padding.
    #[must_use]
    pub const fn bytes(self) -> &'input [u8] {
        self.bytes
    }

    /// Returns the physical stream payload within the containing bytes.
    #[must_use]
    pub const fn payload_range(self) -> Range<u64> {
        let start = self.header.start + 2 * WORD_BYTES;
        start..start + self.bytes.len() as u64
    }

    /// Returns the available padding after the stream's declared elements.
    #[must_use]
    pub const fn padding_range(self) -> Range<u64> {
        self.payload_range().end..self.padding_end
    }
}

/// An occurrence encountered while walking original stream headers.
#[derive(Debug)]
#[non_exhaustive]
pub enum MeshStreamOccurrence<'input> {
    /// The full payload fits under its count convention.
    Complete(MeshStreamSource<'input>),
    /// The header fits but its count-driven payload crosses the end of the chunk.
    Incomplete(MeshStreamHeader),
}

pub(crate) enum StreamStep<'input> {
    Complete(MeshStreamSource<'input>),
    Incomplete(MeshStreamHeader, ParseError),
}

impl<'input> StreamStep<'input> {
    pub(crate) const fn disclosed(self) -> MeshStreamOccurrence<'input> {
        match self {
            Self::Complete(stream) => MeshStreamOccurrence::Complete(stream),
            Self::Incomplete(header, _) => MeshStreamOccurrence::Incomplete(header),
        }
    }
}

pub(crate) struct StreamScan<'input> {
    reader: ByteReader<'input>,
    vertices: u32,
    indices: u32,
    ended: bool,
}

impl<'input> StreamScan<'input> {
    pub(crate) const fn new(reader: ByteReader<'input>, vertices: u32, indices: u32) -> Self {
        Self {
            reader,
            vertices,
            indices,
            ended: false,
        }
    }

    fn next_source(&mut self) -> Result<Option<StreamStep<'input>>, ParseError> {
        while self.reader.remaining() >= size_of::<u32>() {
            let start = self.reader.offset();
            let tag = self.reader.read_u32()?;
            if tag == 0 {
                continue;
            }
            let stride = self.reader.read_u32()?;
            let count = if tag == INDICES {
                self.indices
            } else {
                self.vertices
            };
            let header = MeshStreamHeader {
                tag,
                stride,
                count,
                start,
            };
            let required = u64::from(count) * u64::from(stride);
            let data = self.take_payload(required);
            if data.is_err() {
                self.ended = true;
            }
            return Ok(Some(match data {
                Ok(bytes) => StreamStep::Complete(MeshStreamSource {
                    header,
                    bytes,
                    padding_end: self.reader.offset(),
                }),
                Err(error) => StreamStep::Incomplete(header, error),
            }));
        }
        Ok(None)
    }

    fn take_payload(&mut self, required: u64) -> Result<&'input [u8], ParseError> {
        let length = usize::try_from(required).map_err(|_| ParseError::UnexpectedEnd {
            offset: self.reader.offset(),
            needed: required,
            available: self.reader.remaining() as u64,
        })?;
        let bytes = self.reader.read_bytes(length)?;
        let padding = (ALIGNMENT - length % ALIGNMENT) % ALIGNMENT;
        self.reader.skip(padding.min(self.reader.remaining()))?;
        Ok(bytes)
    }
}

impl<'input> Iterator for StreamScan<'input> {
    type Item = Result<StreamStep<'input>, CorruptChunk>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        let source = self.next_source();
        self.ended |= !matches!(&source, Ok(Some(_)));
        source.transpose().map(|row| row.map_err(Into::into))
    }
}
