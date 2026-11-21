// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek, Write};
use std::ops::Range;

use thiserror::Error;

use super::WemInspection;
use crate::adpcm::Layout;
use crate::read_at::SeekSource;
use crate::{AudioError, ByteOrder, Codec, WavReport, WemMetadata, wav};

/// A physical I/O failure during seekable PCM WAV output.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WemPcmWriteError {
    /// The original audio range could not be read.
    #[error("cannot read the original WEM data")]
    Input(#[source] io::Error),
    /// The converted WAV bytes could not be written.
    #[error("cannot write the converted WAV")]
    Output(#[source] io::Error),
}

/// Prepared 16-bit PCM WAV output over one checked original source range.
pub struct WemPcmWav<'source, R> {
    input: &'source mut SeekSource<R>,
    metadata: &'source WemMetadata,
    range: Range<u64>,
    decoder: Decoder,
    header: Vec<u8>,
    report: WavReport,
}

impl<R> WemPcmWav<'_, R> {
    /// Returns the metadata that selected the original source range.
    #[must_use]
    pub const fn metadata(&self) -> &WemMetadata {
        self.metadata
    }

    /// Returns the report for a successful write.
    #[must_use]
    pub const fn report(&self) -> WavReport {
        self.report
    }

    /// Returns the WEM-relative half-open byte range consumed by this conversion.
    #[must_use]
    pub fn source_range(&self) -> Range<u64> {
        self.range.clone()
    }
}

impl<R: Read + Seek> WemPcmWav<'_, R> {
    /// Writes canonical little-endian WAV with block-aligned bounded transfers.
    ///
    /// # Errors
    /// Returns the original read or write failure, which can leave the sink partially written.
    pub fn write(self, output: &mut (impl Write + ?Sized)) -> Result<(), WemPcmWriteError> {
        output
            .write_all(&self.header)
            .map_err(WemPcmWriteError::Output)?;
        let transfer = self.decoder.transfer_bytes();
        let mut position = self.range.start;
        let mut buffer = vec![0; buffer_length(self.range.end - position, transfer)];
        let mut decoded = Vec::new();
        while position < self.range.end {
            let count = buffer_length(self.range.end - position, transfer);
            let chunk = buffer.get_mut(..count).ok_or_else(|| {
                WemPcmWriteError::Input(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "the WAV transfer buffer is out of bounds",
                ))
            })?;
            self.input
                .read_io_at(position, chunk)
                .map_err(WemPcmWriteError::Input)?;
            self.decoder
                .write(chunk, self.metadata.byte_order(), &mut decoded, output)
                .map_err(WemPcmWriteError::Output)?;
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
        if self.metadata.codec() != Codec::Pcm {
            return Err(AudioError::UnsupportedCodec(self.metadata.codec()));
        }
        self.wav()
    }

    /// Prepares canonical WAV from this inspected PCM, IMA ADPCM or PTADPCM source.
    ///
    /// # Errors
    /// Returns an unsupported codec, invalid block layout or unrepresentable WAV size.
    pub fn wav(&mut self) -> Result<WemPcmWav<'_, R>, AudioError> {
        let metadata = &self.metadata;
        let data = metadata.data_chunk();
        let selected = SelectedInput::new(metadata, data.body_len)?;
        let (header, report) = metadata.decoded_wav_header(selected.frames)?;
        let end = data
            .body_offset
            .checked_add(selected.bytes)
            .filter(|end| *end <= data.body_end_exclusive())
            .ok_or(AudioError::InvalidFormat(
                "the decoded input exceeds the original data range",
            ))?;
        Ok(WemPcmWav {
            input: &mut self.source.input,
            metadata,
            range: data.body_offset..end,
            decoder: selected.decoder,
            header,
            report,
        })
    }
}

#[derive(Clone, Copy)]
enum Decoder {
    Pcm,
    Adpcm(Layout),
}

impl Decoder {
    const fn transfer_bytes(self) -> usize {
        let alignment = match self {
            Self::Pcm => size_of::<i16>(),
            Self::Adpcm(layout) => layout.block_len(),
        };
        wav::PCM_TRANSFER_BYTES / alignment * alignment
    }

    fn write(
        self,
        data: &mut [u8],
        order: ByteOrder,
        decoded: &mut Vec<u8>,
        output: &mut (impl Write + ?Sized),
    ) -> io::Result<()> {
        match self {
            Self::Pcm => {
                if order == ByteOrder::Big {
                    wav::swap_pcm_samples(data);
                }
                output.write_all(data)
            }
            Self::Adpcm(layout) => layout.write_pcm(data, order, decoded, output),
        }
    }
}

struct SelectedInput {
    decoder: Decoder,
    frames: u64,
    bytes: u64,
}

impl SelectedInput {
    fn new(metadata: &WemMetadata, data_len: u64) -> Result<Self, AudioError> {
        match metadata.codec() {
            Codec::Pcm => {
                let frames = metadata.sample_count().ok_or(AudioError::InvalidFormat(
                    "the selected PCM source has no frame count",
                ))?;
                let bytes = frames
                    .checked_mul(u64::from(metadata.block_align()))
                    .ok_or(AudioError::WavTooLarge)?;
                Ok(Self {
                    decoder: Decoder::Pcm,
                    frames,
                    bytes,
                })
            }
            codec @ (Codec::ImaAdpcm | Codec::PtAdpcm) => {
                let layout = Layout::of(codec, metadata.channels(), metadata.block_align()).ok_or(
                    AudioError::InvalidFormat(
                        "the ADPCM block alignment does not fit the channels",
                    ),
                )?;
                Ok(Self {
                    decoder: Decoder::Adpcm(layout),
                    frames: layout.frames(data_len),
                    bytes: layout.consumed_bytes(data_len),
                })
            }
            codec => Err(AudioError::UnsupportedCodec(codec)),
        }
    }
}

fn buffer_length(bytes: u64, transfer: usize) -> usize {
    usize::try_from(bytes).unwrap_or(usize::MAX).min(transfer)
}
