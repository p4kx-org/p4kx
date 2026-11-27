// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek};
use std::sync::Arc;

use thiserror::Error;

use crate::read_at::SeekSource;
use crate::riff::{RiffDirectory, RiffHeader, TailPolicy};
use crate::{
    AudioError, ByteOrder, WemCue, WemList, WemMetadata, WemOpusOgg, WemOpusWav, WemSourceChunk,
    WemVorbisOgg, WemVorbisWav,
};

/// A failure to inspect a seekable WEM source.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WemReadError {
    /// A WEM format declaration is missing, malformed or unsupported.
    #[error(transparent)]
    Format(#[from] AudioError),
    /// The supplied input failed a physical read or seek.
    #[error("cannot read the WEM source")]
    Io(#[from] io::Error),
    /// The supplied cursor or occurrence does not identify the expected source or chunk.
    #[error("the RIFF cursor or chunk does not identify the expected source occurrence")]
    InvalidOccurrence,
    /// The supplied text buffer cannot hold exactly the original text range.
    #[error("the text buffer length differs from the original RIFF text range")]
    TextBufferLength,
}

/// Fixed-memory inspection of a WEM in a caller-owned seekable byte source.
pub struct WemSource<R> {
    input: SeekSource<R>,
    header: RiffHeader,
    owner: Arc<()>,
}

#[path = "wem_pcm_wav.rs"]
mod pcm_wav;

pub use pcm_wav::{WemPcmWav, WemPcmWriteError};

/// Decoded WEM metadata bound to its actual seekable source.
pub struct WemInspection<'source, R> {
    source: &'source mut WemSource<R>,
    metadata: WemMetadata,
}

impl<R> WemInspection<'_, R> {
    /// Borrows the metadata and original occurrences from this inspection.
    #[must_use]
    pub const fn metadata(&self) -> &WemMetadata {
        &self.metadata
    }
}

impl<R: Read + Seek> WemInspection<'_, R> {
    /// Prepares Ogg output from this inspected Wwise Vorbis source.
    ///
    /// # Errors
    /// Returns the original format or physical input failure before output is written.
    pub fn vorbis_ogg(&mut self) -> Result<WemVorbisOgg<'_, R>, WemReadError> {
        crate::vorbis::prepare_ogg(&mut self.source.input, self.source.header, &self.metadata)
    }

    /// Prepares 16-bit PCM WAV from this inspected Wwise Vorbis source.
    ///
    /// # Errors
    /// Returns the original source failure or a WAV size error before writing.
    pub fn vorbis_wav(&mut self) -> Result<WemVorbisWav<'_, R>, WemReadError> {
        crate::vorbis::prepare_wav(&mut self.source.input, self.source.header, &self.metadata)
    }

    /// Prepares Ogg output from this inspected Opus source.
    ///
    /// # Errors
    /// Returns the original format or physical input failure before output is written.
    pub fn opus_ogg(&mut self) -> Result<WemOpusOgg<'_, R>, WemReadError> {
        crate::opus::prepare_ogg(&mut self.source.input, self.source.header, &self.metadata)
    }

    /// Prepares 16-bit PCM WAV from this inspected Opus source.
    ///
    /// # Errors
    /// Returns the original source failure or a WAV size error before writing.
    pub fn opus_wav(&mut self) -> Result<WemOpusWav<'_, R>, WemReadError> {
        crate::opus::prepare_wav(&mut self.source.input, self.source.header, &self.metadata)
    }
}

/// An independent cursor bound to one WEM source.
pub struct WemChunkCursor {
    directory: RiffDirectory,
    owner: Arc<()>,
}

/// A physical chunk read from the directory of one WEM source.
#[derive(Debug, Clone)]
pub struct WemChunkEntry {
    chunk: WemSourceChunk,
    owner: Arc<()>,
}

impl WemChunkEntry {
    /// Returns the original chunk header and byte coordinates.
    #[must_use]
    pub const fn chunk(&self) -> WemSourceChunk {
        self.chunk
    }

    fn require(&self, owner: &Arc<()>, id: [u8; 4]) -> Result<WemSourceChunk, WemReadError> {
        if Arc::ptr_eq(&self.owner, owner) && self.chunk.id == id {
            Ok(self.chunk)
        } else {
            Err(WemReadError::InvalidOccurrence)
        }
    }
}

impl<R: Read + Seek> WemSource<R> {
    /// Opens the RIFF header within the supplied exact source length.
    ///
    /// # Errors
    /// Returns a format error for an invalid header or an input read/seek error.
    pub fn open(reader: R, byte_length: u64) -> Result<Self, WemReadError> {
        let mut input = SeekSource::new(reader, byte_length);
        let header = RiffHeader::read(&mut input)?;
        Ok(Self {
            input,
            header,
            owner: Arc::new(()),
        })
    }

    /// Reads the summary and original decisions from the first data and preceding declarations.
    ///
    /// # Errors
    /// Returns an error for incomplete required chunks, invalid format or an input read/seek failure.
    pub fn read_metadata(&mut self) -> Result<WemMetadata, WemReadError> {
        WemMetadata::read(&mut self.input, self.header)
    }

    /// Inspects the selected WEM metadata and binds subsequent reads to the same source.
    ///
    /// # Errors
    /// Returns the original WEM metadata or input failure.
    pub fn inspect(&mut self) -> Result<WemInspection<'_, R>, WemReadError> {
        let metadata = self.read_metadata()?;
        Ok(WemInspection {
            source: self,
            metadata,
        })
    }

    /// Returns the original byte order declared by the RIFF signature.
    #[must_use]
    pub const fn byte_order(&self) -> ByteOrder {
        self.header.order
    }

    /// Returns the exclusive declared RIFF end in source-relative bytes.
    #[must_use]
    pub const fn riff_end(&self) -> u64 {
        self.header.end
    }

    /// Starts an independent scan of the checked RIFF directory.
    #[must_use]
    pub fn chunk_cursor(&self) -> WemChunkCursor {
        WemChunkCursor {
            directory: self.header.directory(TailPolicy::Checked),
            owner: Arc::clone(&self.owner),
        }
    }

    /// Reads the next checked physical chunk and retains its source binding.
    pub fn next_chunk(
        &mut self,
        cursor: &mut WemChunkCursor,
    ) -> Option<Result<WemChunkEntry, WemReadError>> {
        if !Arc::ptr_eq(&self.owner, &cursor.owner) {
            return Some(Err(WemReadError::InvalidOccurrence));
        }
        cursor.directory.next(&mut self.input).map(|item| {
            item.map(|chunk| WemChunkEntry {
                chunk,
                owner: Arc::clone(&self.owner),
            })
        })
    }

    /// Visits the original records in one physical cue chunk.
    ///
    /// # Errors
    /// Returns an error for a foreign or wrong chunk or an unreadable count.
    pub fn cue(&mut self, entry: &WemChunkEntry) -> Result<WemCue<'_, R>, WemReadError> {
        let chunk = entry.require(&self.owner, *b"cue ")?;
        WemCue::open(&mut self.input, self.header.order, chunk)
    }

    /// Visits the original subchunks in one physical LIST chunk.
    ///
    /// # Errors
    /// Returns an error for a foreign or wrong chunk or an unreadable list type.
    pub fn list(&mut self, entry: &WemChunkEntry) -> Result<WemList<'_, R>, WemReadError> {
        let chunk = entry.require(&self.owner, *b"LIST")?;
        WemList::open(&mut self.input, self.header.order, chunk, &self.owner)
    }

    /// Visits the complete checked RIFF directory in original order, including chunks after data.
    /// An incomplete header or body is returned as the last item.
    pub fn chunks(&mut self) -> impl Iterator<Item = Result<WemSourceChunk, WemReadError>> + '_ {
        let mut directory = self.header.directory(TailPolicy::Checked);
        std::iter::from_fn(move || directory.next(&mut self.input))
    }
}
