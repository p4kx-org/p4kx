// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufReader, Read};

use flate2::read::DeflateDecoder;

use crate::decode::{CorruptAs, ZstdOrPlain};
use crate::format_error::FormatError;
use crate::{Compression, EffectiveEncoding, Entry};

pub(crate) type DataReader<'a> = Box<dyn Read + Send + 'a>;

type Deflate<'a> = CorruptAs<DeflateDecoder<DataReader<'a>>>;
type Zstandard<'a> = CorruptAs<ZstdOrPlain<BufReader<DataReader<'a>>>>;

pub(crate) enum EntryDecoder<'a> {
    Stored(DataReader<'a>),
    Deflate(Box<Deflate<'a>>),
    Zstandard(Box<Zstandard<'a>>),
}

impl<'a> EntryDecoder<'a> {
    pub(crate) fn new(
        stored: DataReader<'a>,
        entry: &Entry,
        buffer_len: usize,
    ) -> Result<Self, FormatError> {
        Ok(match entry.compression() {
            Compression::Store => Self::Stored(stored),
            Compression::Deflate => Self::Deflate(Box::new(CorruptAs::new(
                DeflateDecoder::new_with_buf(stored, vec![0; buffer_len]),
                entry.compression(),
            ))),
            Compression::Zstd => Self::Zstandard(Box::new(CorruptAs::new(
                ZstdOrPlain::new(
                    BufReader::with_capacity(buffer_len, stored),
                    entry.uncompressed_size(),
                ),
                entry.compression(),
            ))),
            Compression::Other(_) => {
                return Err(FormatError::UnsupportedCompression {
                    entry: entry.path().to_owned(),
                    compression: entry.compression(),
                });
            }
        })
    }

    pub(crate) fn effective_encoding(&self) -> EffectiveEncoding {
        match self {
            Self::Stored(_) => EffectiveEncoding::Stored,
            Self::Deflate(_) => EffectiveEncoding::Deflate,
            Self::Zstandard(reader) => reader.inner().effective_encoding(),
        }
    }
}

impl Read for EntryDecoder<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Stored(reader) => reader.read(buffer),
            Self::Deflate(reader) => reader.read(buffer),
            Self::Zstandard(reader) => reader.read(buffer),
        }
    }
}
