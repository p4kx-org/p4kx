// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek, Write};
use std::ops::Range;

use thiserror::Error;

use super::OggReport;
use super::header::{self, AVERAGE_BYTES_OFFSET, Header, VORB_LEN};
use super::seek_packets::SeekAudio;
use super::stream::{Headers, StreamError};
use crate::read_at::{ReadAt, SeekSource};
use crate::riff::RiffHeader;
use crate::wem_metadata_read::selected_chunks;
use crate::{AudioError, Codec, WemMetadata, WemReadError};

#[path = "seek_wav.rs"]
mod wav;

pub use wav::WemVorbisWav;
pub(crate) use wav::prepare as prepare_wav;

/// Physical input extents for one prepared Wwise Vorbis conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WemVorbisRanges {
    /// Original average byte rate in the selected format declaration.
    pub average_byte_rate: Range<u64>,
    /// Original Wwise Vorbis configuration bytes.
    pub header: Range<u64>,
    /// Original setup packet, including its size prefix.
    pub setup_packet: Range<u64>,
    /// Original audio packets, including size prefixes and empty packet selectors.
    pub audio_packets: Range<u64>,
}

/// Failure during a prepared seekable Vorbis write.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WemVorbisWriteError {
    /// The original source failed a physical read or seek.
    #[error("cannot read the original Wwise Vorbis packets")]
    Input(#[source] io::Error),
    /// A replayed packet is invalid.
    #[error(transparent)]
    Format(#[from] AudioError),
    /// The converted bytes could not be written.
    #[error("cannot write the converted Vorbis audio")]
    Output(#[source] io::Error),
    /// PCM replay used a different packet range from the prepared source.
    #[error("Vorbis replay ended at WEM byte {actual}, expected {expected}")]
    ReplayRangeChanged {
        /// Expected exclusive end in source-relative bytes.
        expected: u64,
        /// Actual exclusive end in source-relative bytes.
        actual: u64,
    },
}

impl From<StreamError<io::Error>> for WemVorbisWriteError {
    fn from(error: StreamError<io::Error>) -> Self {
        match error {
            StreamError::Input(error) => Self::Input(error),
            StreamError::Audio(AudioError::Write(error)) => Self::Output(error),
            StreamError::Audio(error) => Self::Format(error),
        }
    }
}

/// Prepared Ogg Vorbis output over a checked original Wwise Vorbis source.
pub struct WemVorbisOgg<'source, R>(PreparedVorbis<'source, R>);

struct PreparedVorbis<'source, R> {
    source: SeekAudio<'source, R>,
    metadata: &'source WemMetadata,
    header: Header,
    headers: Headers,
    ranges: WemVorbisRanges,
}

impl<R> WemVorbisOgg<'_, R> {
    /// Returns the source metadata used to prepare the conversion.
    #[must_use]
    pub const fn metadata(&self) -> &WemMetadata {
        self.0.metadata
    }

    /// Returns the original conversion input extents relative to the start of the WEM.
    #[must_use]
    pub const fn ranges(&self) -> &WemVorbisRanges {
        &self.0.ranges
    }

    /// Returns the number of output frames after applying the authored sample count.
    #[must_use]
    pub const fn sample_count(&self) -> u64 {
        self.0.headers.sample_count
    }
}

impl<R: Read + Seek> WemVorbisOgg<'_, R> {
    /// Writes deterministic Ogg Vorbis using bounded original packet buffers.
    ///
    /// # Errors
    /// Returns the original input, format or output failure. The sink can be partially written.
    pub fn write(
        mut self,
        output: &mut (impl Write + ?Sized),
    ) -> Result<OggReport, WemVorbisWriteError> {
        let prepared = &mut self.0;
        Ok(prepared
            .headers
            .write(&mut prepared.source, &prepared.header, output)?)
    }
}

pub(crate) fn prepare_ogg<'source, R: Read + Seek>(
    input: &'source mut SeekSource<R>,
    riff: RiffHeader,
    metadata: &'source WemMetadata,
) -> Result<WemVorbisOgg<'source, R>, WemReadError> {
    if metadata.codec() != Codec::Vorbis {
        return Err(AudioError::UnsupportedOggCodec(metadata.codec()).into());
    }
    prepare(input, riff, metadata).map(WemVorbisOgg)
}

fn prepare<'source, R: Read + Seek>(
    input: &'source mut SeekSource<R>,
    riff: RiffHeader,
    metadata: &'source WemMetadata,
) -> Result<PreparedVorbis<'source, R>, WemReadError> {
    let HeaderRead {
        header,
        average_byte_rate,
        header_range,
    } = read_header(input, riff, metadata)?;
    let data = metadata.data_chunk();
    let mut source = SeekAudio {
        input,
        data: data.body_offset..data.body_end_exclusive(),
        order: riff.order,
    };
    let mut setup = Vec::new();
    let setup_packet = source.prepare(&header, &mut setup).map_err(read_error)?;
    let headers = Headers::rebuild(&mut source, &header, &setup, metadata.loop_points())
        .map_err(read_error)?;
    let ranges = WemVorbisRanges {
        average_byte_rate,
        header: header_range,
        setup_packet,
        audio_packets: source.data.start + header.audio_offset as u64..source.data.end,
    };
    Ok(PreparedVorbis {
        source,
        metadata,
        header,
        headers,
        ranges,
    })
}

fn read_error(error: StreamError<io::Error>) -> WemReadError {
    match error {
        StreamError::Audio(error) => error.into(),
        StreamError::Input(error) => error.into(),
    }
}

struct HeaderRead {
    header: Header,
    average_byte_rate: Range<u64>,
    header_range: Range<u64>,
}

fn read_header<R: Read + Seek>(
    input: &mut SeekSource<R>,
    riff: RiffHeader,
    metadata: &WemMetadata,
) -> Result<HeaderRead, WemReadError> {
    let selected = selected_chunks(input, riff)?;
    let fmt = selected.format;
    let chunk_len = |len| usize::try_from(len).unwrap_or(usize::MAX);
    let offset = header::configuration_offset(
        chunk_len(fmt.body_len),
        selected.vorbis.map(|chunk| chunk_len(chunk.body_len)),
    )
    .map_err(AudioError::InvalidVorbis)?;
    let vorb_offset = selected.vorbis.unwrap_or(fmt).body_offset + offset as u64;
    let rate_offset = fmt.body_offset + AVERAGE_BYTES_OFFSET as u64;
    let mut average = [0; size_of::<u32>()];
    input.read_at(rate_offset, &mut average)?;
    let mut vorb = [0; VORB_LEN];
    input.read_at(vorb_offset, &mut vorb)?;
    let header = Header::parse(
        riff.order,
        riff.order.u32(average),
        &vorb,
        metadata.channels(),
        metadata.sample_rate(),
    )
    .map_err(AudioError::InvalidVorbis)?;
    Ok(HeaderRead {
        header,
        average_byte_rate: rate_offset..rate_offset + average.len() as u64,
        header_range: vorb_offset..vorb_offset + VORB_LEN as u64,
    })
}
