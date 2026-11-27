// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::io::{self, Read};

use flate2::read::ZlibDecoder;
use lzma_rs::decompress::{Options, UnpackedSize};
use p4kx_foundation::ParseError;

use crate::geometry::Rect;
use crate::reader::TagReader;
use crate::{GfxError, MovieSource, MovieSpan};

/// The length of the file header before the movie header: signature, version and length.
pub(crate) const FILE_HEADER_LEN: usize = 8;

/// The largest movie decompressed into memory, far above the largest movie of a real Archive.
pub const MAX_MOVIE_LEN: u64 = 64 << 20;

const LZMA_PROPERTIES_START: usize = 12;

/// The signature a movie starts with, naming its container and compression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Signature {
    /// An uncompressed SWF movie, `FWS`.
    Fws,
    /// A zlib-compressed SWF movie, `CWS`.
    Cws,
    /// An LZMA-compressed SWF movie, `ZWS`.
    Zws,
    /// An uncompressed Scaleform GFx movie, `GFX`.
    Gfx,
    /// A zlib-compressed Scaleform GFx movie, `CFX`.
    Cfx,
}

/// How the movie after the file header is compressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Compression {
    /// Not compressed.
    None,
    /// zlib.
    Zlib,
    /// LZMA.
    Lzma,
}

impl Signature {
    const fn parse(bytes: &[u8]) -> Option<Self> {
        match bytes {
            b"FWS" => Some(Self::Fws),
            b"CWS" => Some(Self::Cws),
            b"ZWS" => Some(Self::Zws),
            b"GFX" => Some(Self::Gfx),
            b"CFX" => Some(Self::Cfx),
            _ => None,
        }
    }

    /// Returns the three signature characters.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fws => "FWS",
            Self::Cws => "CWS",
            Self::Zws => "ZWS",
            Self::Gfx => "GFX",
            Self::Cfx => "CFX",
        }
    }

    /// Returns whether the movie is a Scaleform GFx movie rather than a plain SWF movie.
    #[must_use]
    pub const fn is_gfx(self) -> bool {
        matches!(self, Self::Gfx | Self::Cfx)
    }

    /// Returns how the movie after the file header is compressed.
    #[must_use]
    pub const fn compression(self) -> Compression {
        match self {
            Self::Fws | Self::Gfx => Compression::None,
            Self::Cws | Self::Cfx => Compression::Zlib,
            Self::Zws => Compression::Lzma,
        }
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The file and movie header of a GFx or SWF movie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Header {
    /// The signature.
    pub signature: Signature,
    /// The SWF or GFx version.
    pub version: u8,
    /// The uncompressed length the file header declares, file header included.
    pub declared_len: u32,
    /// The stage in twips.
    pub frame_size: Rect,
    /// The frames per second.
    pub frame_rate: f32,
    /// The frames of the main timeline the header declares.
    pub frame_count: u16,
}

pub(crate) struct DecodedMovie {
    pub(crate) header: Header,
    pub(crate) data: Vec<u8>,
    pub(crate) tags: usize,
    pub(crate) source: MovieSource,
}

pub(crate) fn read_movie(bytes: Vec<u8>) -> Result<DecodedMovie, GfxError> {
    let (signature, version, declared_len) = read_file_header(&bytes)?;
    let declared = usize::try_from(declared_len).unwrap_or(usize::MAX);
    let input_length = bytes.len();
    let (data, compressed) = match signature.compression() {
        Compression::None => {
            let mut bytes = bytes;
            bytes.truncate(declared);
            (bytes, None)
        }
        compression => {
            let (data, consumed) = decompress(&bytes, declared, compression)?;
            (data, Some(consumed))
        }
    };
    let body = data.get(FILE_HEADER_LEN..).unwrap_or_default();
    let mut reader = TagReader::new(body, FILE_HEADER_LEN);
    let frame_size = reader.rect()?;
    let stage = MovieSpan::new(FILE_HEADER_LEN, reader.offset());
    let rate = reader.u16()?;
    let frame_rate = MovieSpan::new(stage.end(), reader.offset());
    let frame_count = reader.u16()?;
    let tags = reader.offset();
    Ok(DecodedMovie {
        header: Header {
            signature,
            version,
            declared_len,
            frame_size,
            frame_rate: f32::from(rate >> 8) + f32::from(rate & 0xFF) / 256.0,
            frame_count,
        },
        data,
        tags,
        source: MovieSource {
            signature: MovieSpan::new(0, 3),
            version: MovieSpan::new(3, 4),
            declared_length: MovieSpan::new(4, FILE_HEADER_LEN),
            input_length,
            compressed,
            stage,
            frame_rate,
            frame_count: MovieSpan::new(frame_rate.end(), tags),
        },
    })
}

/// Reads the signature, the version and the declared uncompressed length of the file header.
pub(crate) fn read_file_header(bytes: &[u8]) -> Result<(Signature, u8, u32), GfxError> {
    let Some(file_header) = bytes.get(..FILE_HEADER_LEN) else {
        return Err(GfxError::Parse(ParseError::UnexpectedEnd {
            offset: 0,
            needed: FILE_HEADER_LEN as u64,
            available: bytes.len() as u64,
        }));
    };
    let signature = file_header
        .get(..3)
        .and_then(Signature::parse)
        .ok_or_else(|| GfxError::Signature {
            found: file_header.get(..3).unwrap_or_default().to_vec(),
        })?;
    let mut reader = TagReader::new(file_header, 0);
    reader.bytes(3)?;
    let version = reader.u8()?;
    let declared_len = reader.u32()?;
    if usize::try_from(declared_len).unwrap_or(usize::MAX) < FILE_HEADER_LEN {
        return Err(GfxError::DeclaredLength {
            declared: declared_len,
        });
    }
    Ok((signature, version, declared_len))
}

fn decompress(
    bytes: &[u8],
    declared: usize,
    compression: Compression,
) -> Result<(Vec<u8>, MovieSpan), GfxError> {
    if declared as u64 > MAX_MOVIE_LEN {
        return Err(GfxError::TooLarge {
            limit: MAX_MOVIE_LEN,
        });
    }
    let body_len = declared - FILE_HEADER_LEN;
    let capacity = declared.min(bytes.len().saturating_mul(4));
    let mut data = Vec::with_capacity(capacity);
    data.extend_from_slice(bytes.get(..FILE_HEADER_LEN).unwrap_or_default());
    let compressed = bytes.get(FILE_HEADER_LEN..).unwrap_or_default();
    let consumed = match compression {
        Compression::None | Compression::Zlib => {
            let mut decoder = ZlibDecoder::new(compressed);
            (&mut decoder)
                .take(body_len as u64)
                .read_to_end(&mut data)
                .map_err(|error| decompress_error(&error))?;
            let read = usize::try_from(decoder.total_in()).unwrap_or(compressed.len());
            MovieSpan::new(FILE_HEADER_LEN, FILE_HEADER_LEN + read)
        }
        Compression::Lzma => {
            let mut input = bytes.get(LZMA_PROPERTIES_START..).unwrap_or_default();
            let input_len = input.len();
            let options = Options {
                unpacked_size: UnpackedSize::UseProvided(Some(body_len as u64)),
                memlimit: Some(body_len.max(1)),
                allow_incomplete: false,
            };
            lzma_rs::lzma_decompress_with_options(&mut input, &mut data, &options).map_err(
                |error| GfxError::Decompress {
                    message: error.to_string(),
                },
            )?;
            MovieSpan::new(
                LZMA_PROPERTIES_START,
                LZMA_PROPERTIES_START + input_len - input.len(),
            )
        }
    };
    Ok((data, consumed))
}

fn decompress_error(error: &io::Error) -> GfxError {
    GfxError::Decompress {
        message: error.to_string(),
    }
}
