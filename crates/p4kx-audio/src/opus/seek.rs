// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek, Write};
use std::ops::Range;

use thiserror::Error;

use super::OpusError;
use super::header::{Format, SWITCH_FMT_LEN, StreamHeader};
use super::seek_packets::{Binding, Framing};
use super::stream::{self, PacketCount, PacketSource, WriteError};
use crate::read_at::{ReadAt, SeekSource};
use crate::riff::RiffHeader;
use crate::wem_metadata_read::selected_chunks;
use crate::{AudioError, Codec, OggReport, WemMetadata, WemReadError, WemSourceChunk};

/// Original WEM-relative ranges used for one seekable Ogg Opus conversion.
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

/// Failure during a prepared seekable Ogg Opus write.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WemOpusWriteError {
    /// The original packet or source could not be replayed.
    #[error(transparent)]
    Input(#[from] WemReadError),
    /// The converted bytes could not be written.
    #[error("cannot write the converted Ogg Opus")]
    Output(#[source] io::Error),
}

/// Prepared Ogg Opus output bound to the admitted packet source.
pub struct WemOpusOgg<'source, R> {
    input: &'source mut SeekSource<R>,
    metadata: &'source WemMetadata,
    binding: Binding,
    header: StreamHeader,
    count: PacketCount,
    ranges: WemOpusRanges,
}

impl<R> WemOpusOgg<'_, R> {
    /// Returns the selected metadata of this original source.
    #[must_use]
    pub const fn metadata(&self) -> &WemMetadata {
        self.metadata
    }

    /// Returns exact physical extents of the selected packet source.
    #[must_use]
    pub const fn ranges(&self) -> &WemOpusRanges {
        &self.ranges
    }

    /// Returns the output frame count after applying pre-skip and any authored trim.
    #[must_use]
    pub const fn sample_count(&self) -> u64 {
        self.count.frames
    }
}

impl<R: Read + Seek> WemOpusOgg<'_, R> {
    /// Writes deterministic Ogg Opus using bounded packet and Ogg page buffers.
    ///
    /// # Errors
    /// Returns original input or sink failure. A failure can leave the sink partially written.
    pub fn write(self, output: &mut (impl Write + ?Sized)) -> Result<OggReport, WemOpusWriteError> {
        let mut packets = self.binding.packets(self.input);
        packets.skip_embedded_headers()?;
        stream::write(
            &self.header,
            &self.count,
            &mut packets,
            self.metadata.loop_points(),
            output,
        )
        .map_err(|cause| match cause {
            WriteError::Input(cause) => WemOpusWriteError::Input(cause),
            WriteError::Output(cause) => WemOpusWriteError::Output(cause),
        })
    }
}

pub(crate) fn prepare<'source, R: Read + Seek>(
    input: &'source mut SeekSource<R>,
    riff: RiffHeader,
    metadata: &'source WemMetadata,
) -> Result<WemOpusOgg<'source, R>, WemReadError> {
    if metadata.codec() != Codec::Opus {
        return Err(AudioError::UnsupportedOggCodec(metadata.codec()).into());
    }
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
    let (header, embedded_headers) = if let Some(header) = header {
        (header, None)
    } else {
        let header = packets.embedded(metadata.channels(), declared)?;
        let end = binding.data.start + packets.checked_end();
        (header, Some(binding.data.start..end))
    };
    header.check_channels(metadata.channels())?;
    let count = PacketCount::read(&header, &mut packets)?;
    let start = binding.data.start;
    let ranges = WemOpusRanges {
        format,
        seek_table,
        embedded_headers,
        checked_data: start..start + packets.checked_end(),
        emitted_data: start..start + count.emitted_end,
    };
    Ok(WemOpusOgg {
        input,
        metadata,
        binding,
        header,
        count,
        ranges,
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
