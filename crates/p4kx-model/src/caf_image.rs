// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_foundation::{ByteReader, ParseError};
use thiserror::Error;

const HEADER_LENGTH: usize = 8;
const RECORD_LENGTH: usize = 308;
const PATH_FIELD_LENGTH: usize = 256;
const TRAILER_LENGTH: usize = 8;
const FORMAT_WORD: u32 = 2;

/// The physical CAF index records in a supported image chunk.
#[derive(Debug, Clone, Copy)]
pub struct CafImage<'a> {
    offset: u64,
    records: &'a [u8],
    trailer: &'a [u8],
}

/// An original CAF index record and its position in the image Entry.
#[derive(Debug, Clone, Copy)]
pub struct CafImageRecord<'a> {
    offset: u64,
    bytes: &'a [u8; RECORD_LENGTH],
}

/// A failure to establish the CAF image's counted framing.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CafImageError {
    /// The chunk ends before the next declared field.
    #[error(transparent)]
    Truncated(#[from] ParseError),
    /// The chunk has a different prefix format word.
    #[error("CAF image prefix uses unsupported format word {0:#x}")]
    UnsupportedFormat(u32),
    /// The counted record extent exceeds the addressable range.
    #[error("CAF image record extent cannot be represented")]
    Extent,
    /// Bytes remain after the framed trailer.
    #[error("CAF image has {0} extra bytes beyond its trailer")]
    TrailingBytes(usize),
}

/// The text and evidence of a CAF index record's fixed-width path field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CafImagePath<'a> {
    /// A NUL-terminated UTF-8 path.
    Text(&'a str),
    /// The field contains no terminator.
    Unterminated,
    /// The bytes before the terminator are not UTF-8.
    InvalidEncoding,
}

impl<'a> CafImage<'a> {
    /// Reads the version 0x902 CAF image chunk at its original Entry offset.
    ///
    /// # Errors
    /// Returns an error when the counted table and trailer do not frame the chunk.
    pub fn read(bytes: &'a [u8], offset: u64) -> Result<Self, CafImageError> {
        offset
            .checked_add(bytes.len() as u64)
            .ok_or(CafImageError::Extent)?;
        let mut reader = ByteReader::with_base(bytes, offset);
        let count = reader.read_u32()?;
        let format = reader.read_u32()?;
        if format != FORMAT_WORD {
            return Err(CafImageError::UnsupportedFormat(format));
        }
        let table_length = (count as usize)
            .checked_mul(RECORD_LENGTH)
            .ok_or(CafImageError::Extent)?;
        let records = reader.read_bytes(table_length)?;
        let trailer = reader.read_bytes(TRAILER_LENGTH)?;
        if reader.remaining() != 0 {
            return Err(CafImageError::TrailingBytes(reader.remaining()));
        }
        Ok(Self {
            offset,
            records,
            trailer,
        })
    }

    /// Returns the original count and format-word interval.
    #[must_use]
    pub const fn header_range(&self) -> Range<u64> {
        self.offset..self.offset + HEADER_LENGTH as u64
    }

    /// Returns the number of physical CAF records.
    #[must_use]
    pub const fn record_count(&self) -> usize {
        self.records.len() / RECORD_LENGTH
    }

    /// Returns the end-exclusive interval of the original trailer.
    #[must_use]
    pub const fn trailer_range(&self) -> Range<u64> {
        let start = self.header_range().end + self.records.len() as u64;
        start..start + self.trailer.len() as u64
    }

    /// Visits physical records in source order without allocating.
    pub fn records(&self) -> impl ExactSizeIterator<Item = CafImageRecord<'a>> + use<'a> {
        let start = self.header_range().end;
        let (chunks, _) = self.records.as_chunks::<RECORD_LENGTH>();
        chunks
            .iter()
            .enumerate()
            .map(move |(ordinal, bytes)| CafImageRecord {
                offset: start + ordinal as u64 * RECORD_LENGTH as u64,
                bytes,
            })
    }
}

impl<'a> CafImageRecord<'a> {
    /// Returns the end-exclusive interval of the original record.
    #[must_use]
    pub const fn range(&self) -> Range<u64> {
        self.offset..self.offset + self.bytes.len() as u64
    }

    /// Returns the end-exclusive interval of the original fixed-width path field.
    #[must_use]
    pub const fn path_field_range(&self) -> Range<u64> {
        self.offset..self.offset + PATH_FIELD_LENGTH as u64
    }

    /// Returns the source path without consuming its padding.
    #[must_use]
    pub fn path(&self) -> CafImagePath<'a> {
        let Some((field, _)) = self.bytes.split_first_chunk::<PATH_FIELD_LENGTH>() else {
            return CafImagePath::Unterminated;
        };
        let Some(length) = field.iter().position(|byte| *byte == 0) else {
            return CafImagePath::Unterminated;
        };
        let Some(name) = field.get(..length) else {
            return CafImagePath::Unterminated;
        };
        std::str::from_utf8(name).map_or(CafImagePath::InvalidEncoding, CafImagePath::Text)
    }
}
