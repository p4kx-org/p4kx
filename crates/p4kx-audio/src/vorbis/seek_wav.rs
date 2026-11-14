// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek, Write};
use std::ops::Range;

use super::{PreparedVorbis, WemVorbisRanges, WemVorbisWriteError};
use crate::read_at::SeekSource;
use crate::riff::RiffHeader;
use crate::{AudioError, Codec, WavReport, WemMetadata, WemReadError};

/// Prepared 16-bit PCM WAV output bound to the original Wwise Vorbis packets.
pub struct WemVorbisWav<'source, R> {
    source: PreparedVorbis<'source, R>,
    header: Vec<u8>,
    report: WavReport,
    decoded_audio_packets: Range<u64>,
}

impl<R> WemVorbisWav<'_, R> {
    /// Returns the selected metadata of this original source.
    #[must_use]
    pub const fn metadata(&self) -> &WemMetadata {
        self.source.metadata
    }

    /// Returns the report for a successful write.
    #[must_use]
    pub const fn report(&self) -> WavReport {
        self.report
    }

    /// Returns original checked extents, including packet lookahead and framing.
    #[must_use]
    pub const fn ranges(&self) -> &WemVorbisRanges {
        &self.source.ranges
    }

    /// Returns the original audio prefix supplied to the PCM decoder.
    #[must_use]
    pub const fn decoded_audio_packets(&self) -> &Range<u64> {
        &self.decoded_audio_packets
    }
}

impl<R: Read + Seek> WemVorbisWav<'_, R> {
    /// Decodes little-endian PCM WAV using bounded original packet and sample buffers.
    ///
    /// # Errors
    /// Returns the original input, format or output failure. The sink can be partially written.
    pub fn write(mut self, output: &mut (impl Write + ?Sized)) -> Result<(), WemVorbisWriteError> {
        output
            .write_all(&self.header)
            .map_err(WemVorbisWriteError::Output)?;
        let source = &mut self.source;
        let decoded = source
            .headers
            .write_pcm(&mut source.source, &source.header, output)?;
        let actual = source.source.data.start + decoded as u64;
        let expected = self.decoded_audio_packets.end;
        if actual != expected {
            return Err(WemVorbisWriteError::ReplayRangeChanged { expected, actual });
        }
        Ok(())
    }
}

pub(crate) fn prepare<'source, R: Read + Seek>(
    input: &'source mut SeekSource<R>,
    riff: RiffHeader,
    metadata: &'source WemMetadata,
) -> Result<WemVorbisWav<'source, R>, WemReadError> {
    if metadata.codec() != Codec::Vorbis {
        return Err(AudioError::UnsupportedCodec(metadata.codec()).into());
    }
    let source = super::prepare(input, riff, metadata)?;
    let (header, report) = metadata.decoded_wav_header(source.headers.sample_count)?;
    let decoded_audio_packets = source.ranges.audio_packets.start
        ..source.source.data.start + source.headers.decoded_end as u64;
    Ok(WemVorbisWav {
        source,
        header,
        report,
        decoded_audio_packets,
    })
}
