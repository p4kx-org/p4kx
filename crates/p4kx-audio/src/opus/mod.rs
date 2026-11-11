// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod error;
mod framing;
mod header;
mod layout;
mod ogg_read;
mod packets;
mod pcm;
mod seek;
mod seek_packets;
mod seek_pages;
mod stream;

use std::io::Write;

pub use error::OpusError;
pub(crate) use header::TAG_SWITCH;
pub(crate) use seek::prepare as prepare_ogg;
pub use seek::{WemOpusOgg, WemOpusRanges, WemOpusWriteError};

use self::header::{Header, StreamHeader};
use self::packets::{Framing, Packets};
use self::stream::{PacketCount, WriteError};
use crate::riff::{Chunk, Chunks};
use crate::{AudioError, OggReport, Wem};

/// The sample rate every Opus stream decodes at.
pub(crate) const OPUS_RATE: u32 = 48_000;

/// An Opus WEM with every packet checked and counted.
pub(crate) struct OpusStream<'a> {
    data: &'a [u8],
    framing: Framing<'a>,
    header: StreamHeader,
    count: PacketCount,
}

impl<B: AsRef<[u8]>> Wem<B> {
    /// Repackages an Opus sound as a standard Ogg Opus file and writes it to `out`.
    pub(crate) fn write_opus_ogg(&self, out: &mut impl Write) -> Result<OggReport, AudioError> {
        let stream = self.opus_stream()?;
        let mut packets = stream.packets().map_err(AudioError::InvalidOpus)?;
        stream::write(
            &stream.header,
            &stream.count,
            &mut packets,
            self.loop_points(),
            out,
        )
        .map_err(|error| match error {
            WriteError::Input(error) => AudioError::InvalidOpus(error),
            WriteError::Output(error) => AudioError::Write(error),
        })
    }

    /// Reads the header of an Opus WEM and checks and counts its packets.
    pub(crate) fn opus_stream(&self) -> Result<OpusStream<'_>, AudioError> {
        let bytes = self.bytes();
        let (order, chunks) = Chunks::of(bytes)?;
        let mut fmt = None;
        let mut seek = None;
        for chunk in chunks {
            let Chunk { id, body } = chunk?;
            match &id {
                b"fmt " => fmt = bytes.get(body),
                b"seek" => seek = bytes.get(body),
                b"data" => break,
                _ => {}
            }
        }
        let fmt = fmt.ok_or(AudioError::MissingChunk("fmt "))?;
        let header = Header::read(order, fmt, seek, self.data(), self.channels())
            .map_err(AudioError::InvalidOpus)?;
        OpusStream::count(header).map_err(AudioError::InvalidOpus)
    }
}

impl<'a> OpusStream<'a> {
    fn count(header: Header<'a>) -> Result<Self, OpusError> {
        let Header {
            data,
            framing,
            info,
        } = header;
        let mut packets = Packets::new(data, framing)?;
        let count = PacketCount::read(&info, &mut packets)?;
        Ok(Self {
            data,
            framing,
            header: info,
            count,
        })
    }

    /// Returns the frames the stream plays after its pre-skip.
    pub(crate) const fn frames(&self) -> u64 {
        self.count.frames
    }

    fn packets(&self) -> Result<Packets<'a>, OpusError> {
        Packets::new(self.data, self.framing)
    }
}
