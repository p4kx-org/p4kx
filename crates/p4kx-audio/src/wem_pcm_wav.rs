// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek, Write};
use std::ops::Range;

use thiserror::Error;

use super::WemInspection;
use crate::read_at::SeekSource;
use crate::{AudioError, ByteOrder, Codec, WavReport, WemMetadata, wav};

/// A physical I/O failure during seekable PCM WAV output.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WemPcmWriteError {
    /// The original PCM range could not be read.
    #[error("cannot read the original WEM samples")]
    Input(#[source] io::Error),
    /// The converted WAV bytes could not be written.
    #[error("cannot write the converted WAV")]
    Output(#[source] io::Error),
}

/// Prepared WAV output over one source-bound range of original 16-bit PCM frames.
pub struct WemPcmWav<'source, R> {
    input: &'source mut SeekSource<R>,
    metadata: &'source WemMetadata,
    range: Range<u64>,
    byte_order: ByteOrder,
    header: Vec<u8>,
    report: WavReport,
}

impl<R> WemPcmWav<'_, R> {
    /// Returns the metadata that selected the original frame range.
    #[must_use]
    pub const fn metadata(&self) -> &WemMetadata {
        self.metadata
    }

    /// Returns the report for a successful write.
    #[must_use]
    pub const fn report(&self) -> WavReport {
        self.report
    }

    /// Returns the original WEM-relative half-open range of frames used by the output.
    #[must_use]
    pub fn source_range(&self) -> Range<u64> {
        self.range.clone()
    }
}

impl<R: Read + Seek> WemPcmWav<'_, R> {
    /// Writes canonical little-endian WAV using a fixed-size transfer buffer.
    ///
    /// # Errors
    /// Returns the original read or write failure, which can leave the sink partially written.
    pub fn write(self, output: &mut (impl Write + ?Sized)) -> Result<(), WemPcmWriteError> {
        output
            .write_all(&self.header)
            .map_err(WemPcmWriteError::Output)?;
        let mut position = self.range.start;
        let mut buffer = vec![0; buffer_length(self.range.end - position)];
        while position < self.range.end {
            let count = buffer_length(self.range.end - position);
            let chunk = buffer.get_mut(..count).ok_or_else(|| {
                WemPcmWriteError::Input(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "the PCM transfer buffer is out of bounds",
                ))
            })?;
            self.input
                .read_io_at(position, chunk)
                .map_err(WemPcmWriteError::Input)?;
            if self.byte_order == ByteOrder::Big {
                wav::swap_pcm_samples(chunk);
            }
            output.write_all(chunk).map_err(WemPcmWriteError::Output)?;
            position += count as u64;
        }
        Ok(())
    }
}

impl<R: Read + Seek> WemInspection<'_, R> {
    /// Prepares a canonical WAV from the complete PCM frames of this inspected source.
    ///
    /// # Errors
    /// Returns unsupported codec or WAV representation errors before reading frames.
    pub fn pcm_wav(&mut self) -> Result<WemPcmWav<'_, R>, AudioError> {
        let metadata = &self.metadata;
        if metadata.codec() != Codec::Pcm {
            return Err(AudioError::UnsupportedCodec(metadata.codec()));
        }
        let frames = metadata.sample_count().ok_or(AudioError::InvalidFormat(
            "the selected PCM source has no frame count",
        ))?;
        let (header, report) = metadata.decoded_wav_header(frames)?;
        let data = metadata.data_chunk();
        let length = frames
            .checked_mul(u64::from(metadata.block_align()))
            .ok_or(AudioError::WavTooLarge)?;
        let end = data
            .body_offset
            .checked_add(length)
            .filter(|end| *end <= data.body_end_exclusive())
            .ok_or(AudioError::InvalidFormat(
                "the PCM frames exceed their original data range",
            ))?;
        Ok(WemPcmWav {
            input: &mut self.source.input,
            metadata,
            range: data.body_offset..end,
            byte_order: metadata.byte_order(),
            header,
            report,
        })
    }
}

fn buffer_length(bytes: u64) -> usize {
    usize::try_from(bytes)
        .unwrap_or(usize::MAX)
        .min(wav::PCM_TRANSFER_BYTES)
}
