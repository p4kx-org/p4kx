// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek, Write};
use std::ops::Range;

use thiserror::Error;

use super::OpusError;
use super::header::{EmbeddedOpusHeaderPages, Format, SWITCH_FMT_LEN, StreamHeader};
use super::pcm;
use super::seek_packets::{Binding, Framing, SwitchOpusPreSkip};
use super::stream::{self, PacketCount, PacketSource, WriteError};
use crate::ogg::WrittenOgg;
use crate::read_at::{ReadAt, SeekSource};
use crate::riff::RiffHeader;
use crate::wem_metadata_read::selected_chunks;
use crate::{
    AudioError, Codec, OggOutputRanges, OggReport, WavOutputRanges, WavReport, WemMetadata,
    WemReadError, WemSourceChunk,
};

/// Original WEM-relative ranges used for a seekable Opus conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WemOpusRanges {
    /// Checked format prefix covering the original layout, counts and pre-skip selectors.
    pub format: Range<u64>,
    /// Selected packet size words from the standalone seek chunk, when used.
    pub seek_table: Option<Range<u64>>,
    /// Checked embedded Ogg pages containing the original identification and comment packets.
    pub embedded_headers: Option<Range<u64>>,
    /// Checked DATA bytes containing the audio packets and original framing.
    pub checked_data: Range<u64>,
    /// Checked prefix containing every emitted packet, including source framing or page bytes.
    pub emitted_data: Range<u64>,
}

/// Original source ranges and output layout of a successful Ogg Opus write.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WemOpusOggEvidence {
    /// Codec, layout, duration and size of the written Ogg stream.
    pub report: OggReport,
    /// Complete output page ranges.
    pub output: OggOutputRanges,
    /// Checked input extents selected during preparation.
    pub ranges: WemOpusRanges,
    /// Original seek-table size words read during Ogg replay, when present.
    pub replayed_seek_table: Option<Range<u64>>,
    /// Checked original embedded identification and comment page ranges, when present.
    pub embedded_header_pages: Option<EmbeddedOpusHeaderPages>,
    /// Original first-packet header inputs when Switch pre-skip was inferred.
    pub switch_pre_skip: Option<SwitchOpusPreSkip>,
    /// WEM-relative original audio packet bytes replayed, including framing or full checked pages.
    pub replayed_audio_packets: Range<u64>,
}

/// Original source ranges and output layout of a successful Opus WAV write.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WemOpusWavEvidence {
    /// Sample layout and size of the written WAV.
    pub report: WavReport,
    /// Written header and sample byte ranges.
    pub output: WavOutputRanges,
    /// Checked input extents and the DATA prefix consumed during PCM replay.
    pub ranges: WemOpusRanges,
    /// Original seek-table size words read during PCM replay, when present.
    pub replayed_seek_table: Option<Range<u64>>,
}

/// Failure during a prepared seekable Opus output.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WemOpusWriteError {
    /// The original packet or source could not be replayed.
    #[error(transparent)]
    Input(#[from] WemReadError),
    /// The converted bytes could not be written.
    #[error("cannot write the converted Opus audio")]
    Output(#[source] io::Error),
    /// Replay consumed a different original packet extent than preparation selected.
    #[error("Opus replay used {actual} packet bytes, expected {expected}")]
    ReplayRangeChanged {
        /// Number of bytes selected during preparation.
        expected: u64,
        /// Number of bytes covered by the replayed packet prefix.
        actual: u64,
    },
}

impl From<WriteError<WemReadError>> for WemOpusWriteError {
    fn from(error: WriteError<WemReadError>) -> Self {
        match error {
            WriteError::Input(error) => Self::Input(error),
            WriteError::Output(error) => Self::Output(error),
        }
    }
}

/// Prepared Ogg Opus output bound to the admitted packet source.
pub struct WemOpusOgg<'source, R>(PreparedOpus<'source, R>);

/// Prepared 16-bit PCM WAV output bound to the admitted Opus packets.
pub struct WemOpusWav<'source, R> {
    source: PreparedOpus<'source, R>,
    wav_header: Vec<u8>,
    report: WavReport,
}

struct PreparedOpus<'source, R> {
    input: &'source mut SeekSource<R>,
    metadata: &'source WemMetadata,
    binding: Binding,
    header: StreamHeader,
    count: PacketCount,
    ranges: WemOpusRanges,
    embedded_header_pages: Option<EmbeddedOpusHeaderPages>,
    switch_pre_skip: Option<SwitchOpusPreSkip>,
}

impl<R> WemOpusOgg<'_, R> {
    /// Returns the selected metadata of this original source.
    #[must_use]
    pub const fn metadata(&self) -> &WemMetadata {
        self.0.metadata
    }

    /// Returns exact physical extents of the selected packet source.
    #[must_use]
    pub const fn ranges(&self) -> &WemOpusRanges {
        &self.0.ranges
    }

    /// Returns the output frame count after applying pre-skip and any authored trim.
    #[must_use]
    pub const fn sample_count(&self) -> u64 {
        self.0.count.frames
    }
}

impl<R: Read + Seek> WemOpusOgg<'_, R> {
    /// Writes deterministic Ogg Opus using bounded packet and Ogg page buffers.
    ///
    /// # Errors
    /// Returns original input or sink failure. A failure can leave the sink partially written.
    pub fn write(
        mut self,
        output: &mut (impl Write + ?Sized),
    ) -> Result<OggReport, WemOpusWriteError> {
        self.0.write_ogg(output).map(|(written, _)| written.report)
    }

    /// Writes the Ogg stream and returns exact input and output extents.
    ///
    /// # Errors
    /// Returns original input, changed replay extent or sink failure. A failure can leave the sink
    /// partially written.
    pub fn write_with_evidence(
        mut self,
        output: &mut (impl Write + ?Sized),
    ) -> Result<WemOpusOggEvidence, WemOpusWriteError> {
        let source = &mut self.0;
        let (written, replay) = source.write_ogg(output)?;
        check_replay_extent(source.count.emitted_end, replay.data.end)?;
        let data_start = source.binding.data.start;
        Ok(WemOpusOggEvidence {
            report: written.report,
            output: written.output,
            ranges: self.0.ranges,
            embedded_header_pages: self.0.embedded_header_pages,
            switch_pre_skip: self.0.switch_pre_skip,
            replayed_seek_table: replay.seek_table,
            replayed_audio_packets: data_start + replay.data.start..data_start + replay.data.end,
        })
    }
}

struct OggReplay {
    data: Range<u64>,
    seek_table: Option<Range<u64>>,
}

impl<R: Read + Seek> PreparedOpus<'_, R> {
    fn write_ogg(
        &mut self,
        output: &mut (impl Write + ?Sized),
    ) -> Result<(WrittenOgg, OggReplay), WemOpusWriteError> {
        let mut packets = self.binding.audio_packets(self.input)?;
        let written = stream::write(
            &self.header,
            &self.count,
            &mut packets,
            self.metadata.loop_points(),
            output,
        )?;
        let replay = OggReplay {
            data: packets.replayed_audio_start()..packets.checked_end(),
            seek_table: packets.replayed_seek_table(self.ranges.seek_table.as_ref()),
        };
        Ok((written, replay))
    }
}

impl<R> WemOpusWav<'_, R> {
    /// Returns the original metadata used by this WAV.
    #[must_use]
    pub const fn metadata(&self) -> &WemMetadata {
        self.source.metadata
    }

    /// Returns the report for a successful write.
    #[must_use]
    pub const fn report(&self) -> WavReport {
        self.report
    }

    /// Returns the checked physical ranges and the prefix selected for decoding.
    #[must_use]
    pub const fn ranges(&self) -> &WemOpusRanges {
        &self.source.ranges
    }
}

impl<R: Read + Seek> WemOpusWav<'_, R> {
    /// Decodes deterministic little-endian PCM WAV with bounded packet and sample buffers.
    ///
    /// # Errors
    /// Returns the original read, decode or sink failure, which can leave the sink partially written.
    pub fn write(self, output: &mut (impl Write + ?Sized)) -> Result<(), WemOpusWriteError> {
        self.write_with_evidence(output).map(|_| ())
    }

    /// Writes the WAV and returns evidence for the completed output.
    ///
    /// # Errors
    /// Returns the original read, decode or sink failure, which can leave the sink partially written.
    pub fn write_with_evidence(
        self,
        output: &mut (impl Write + ?Sized),
    ) -> Result<WemOpusWavEvidence, WemOpusWriteError> {
        let source = self.source;
        let mut packets = source.binding.audio_packets(source.input)?;
        output
            .write_all(&self.wav_header)
            .map_err(WemOpusWriteError::Output)?;
        pcm::write(&source.header, &source.count, &mut packets, output)?;
        check_replay_extent(source.count.emitted_end, packets.checked_end())?;
        let replayed_seek_table = packets.replayed_seek_table(source.ranges.seek_table.as_ref());
        Ok(WemOpusWavEvidence {
            report: self.report,
            output: WavOutputRanges::new(self.wav_header.len(), self.report),
            ranges: source.ranges,
            replayed_seek_table,
        })
    }
}

const fn check_replay_extent(expected: u64, actual: u64) -> Result<(), WemOpusWriteError> {
    if actual != expected {
        return Err(WemOpusWriteError::ReplayRangeChanged { expected, actual });
    }
    Ok(())
}

pub(crate) fn prepare_ogg<'source, R: Read + Seek>(
    input: &'source mut SeekSource<R>,
    riff: RiffHeader,
    metadata: &'source WemMetadata,
) -> Result<WemOpusOgg<'source, R>, WemReadError> {
    if metadata.codec() != Codec::Opus {
        return Err(AudioError::UnsupportedOggCodec(metadata.codec()).into());
    }
    prepare(input, riff, metadata).map(WemOpusOgg)
}

pub(crate) fn prepare_wav<'source, R: Read + Seek>(
    input: &'source mut SeekSource<R>,
    riff: RiffHeader,
    metadata: &'source WemMetadata,
) -> Result<WemOpusWav<'source, R>, WemReadError> {
    if metadata.codec() != Codec::Opus {
        return Err(AudioError::UnsupportedCodec(metadata.codec()).into());
    }
    let source = prepare(input, riff, metadata)?;
    let (wav_header, report) = metadata.decoded_wav_header(source.count.frames)?;
    Ok(WemOpusWav {
        source,
        wav_header,
        report,
    })
}

fn prepare<'source, R: Read + Seek>(
    input: &'source mut SeekSource<R>,
    riff: RiffHeader,
    metadata: &'source WemMetadata,
) -> Result<PreparedOpus<'source, R>, WemReadError> {
    let Declaration {
        format,
        seek,
        shape,
    } = read_declaration(input, riff, metadata)?;
    let data = metadata.data_chunk();
    let mut binding = Binding {
        data: data.body_offset..data.body_end_exclusive(),
        framing: Framing::Ogg,
    };
    let (seek_table, header, declared) = match shape {
        Format::Table { bytes, info } => {
            let offset = seek
                .ok_or(OpusError::InvalidHeader("the seek chunk is missing"))?
                .body_offset;
            let range = offset..offset + bytes;
            binding.framing = Framing::Table(range.clone());
            (Some(range), Some(info), None)
        }
        Format::Switch { prefix, info } => {
            binding.data.start += prefix;
            binding.framing = Framing::Switch;
            (None, Some(info), None)
        }
        Format::Ogg { declared } => (None, None, declared),
    };
    let mut packets = binding.packets(input);
    let start = binding.data.start;
    let (header, embedded_header_pages) = if let Some(header) = header {
        (header, None)
    } else {
        let (header, pages) = packets.embedded(metadata.channels(), declared)?;
        (header, Some(pages.at(start)))
    };
    let embedded_headers = embedded_header_pages
        .as_ref()
        .map(|_| start..start + packets.checked_end());
    header.check_channels(metadata.channels())?;
    let count = PacketCount::read(&header, &mut packets)?;
    let ranges = WemOpusRanges {
        format,
        seek_table,
        embedded_headers,
        checked_data: start..start + packets.checked_end(),
        emitted_data: start..start + count.emitted_end,
    };
    let switch_pre_skip = packets.switch_pre_skip();
    Ok(PreparedOpus {
        input,
        metadata,
        binding,
        header,
        count,
        ranges,
        embedded_header_pages,
        switch_pre_skip,
    })
}

struct Declaration {
    format: Range<u64>,
    seek: Option<WemSourceChunk>,
    shape: Format,
}

fn read_declaration<R: Read + Seek>(
    input: &mut SeekSource<R>,
    riff: RiffHeader,
    metadata: &WemMetadata,
) -> Result<Declaration, WemReadError> {
    let selected = selected_chunks(input, riff)?;
    let fmt = selected.format;
    let len = (Format::prefix_length(metadata.format_tag()) as u64).min(fmt.body_len);
    let mut prefix = [0; SWITCH_FMT_LEN];
    let bytes = usize::try_from(len)
        .ok()
        .and_then(|len| prefix.get_mut(..len))
        .ok_or(OpusError::InvalidHeader(
            "the checked format prefix is out of bounds",
        ))?;
    input.read_at(fmt.body_offset, bytes)?;
    let shape = Format::read(
        riff.order,
        bytes,
        fmt.body_len,
        selected.seek.map(|chunk| chunk.body_len),
        metadata.data_chunk().body_len,
        metadata.channels(),
    )?;
    Ok(Declaration {
        format: fmt.body_offset..fmt.body_offset + len,
        seek: selected.seek,
        shape,
    })
}

impl From<OpusError> for WemReadError {
    fn from(error: OpusError) -> Self {
        AudioError::InvalidOpus(error).into()
    }
}
