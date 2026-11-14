// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use super::framing;
use super::header::{self, StreamHeader};
use super::packets;
use super::{OPUS_RATE, OpusError};
use crate::loops::insert_comments;
use crate::{AudioError, Codec, Loop, OggReport, OggWriter};

const SERIAL: u32 = 0x5034_4B4F;
const SWITCH_PRE_SKIP_DIVISOR: u32 = 8;
const TAGS_MAGIC_LEN: usize = 8;

pub(super) trait PacketSource {
    type Error: From<OpusError>;

    fn read_packet(&mut self) -> Result<Option<&[u8]>, Self::Error>;

    fn checked_end(&self) -> u64;
}

pub(super) struct PacketCount {
    pub(super) pre_skip: u16,
    pub(super) frames: u64,
    pub(super) emitted_end: u64,
}

impl PacketCount {
    pub(super) fn read<S: PacketSource>(
        header: &StreamHeader,
        source: &mut S,
    ) -> Result<Self, S::Error> {
        let mut total = 0_u64;
        let mut index = 0_u64;
        let mut skip = header.pre_skip;
        let mut emitted_end = None;
        let mut last_packet_end = 0;
        while let Some(packet) = source.read_packet()? {
            let samples = packets::multistream_samples(packet, header.layout.streams, index)?;
            let pre_skip = *skip.get_or_insert_with(|| {
                u16::try_from(samples / SWITCH_PRE_SKIP_DIVISOR).unwrap_or(u16::MAX)
            });
            index += 1;
            total += u64::from(samples);
            last_packet_end = source.checked_end();
            if header
                .declared
                .is_some_and(|frames| total >= frames + u64::from(pre_skip))
            {
                emitted_end.get_or_insert(last_packet_end);
            }
        }
        let pre_skip = skip.unwrap_or_default();
        let available = total
            .checked_sub(u64::from(pre_skip))
            .filter(|&available| available > 0)
            .ok_or(OpusError::NoSamples { pre_skip })?;
        let frames = header
            .declared
            .map_or(available, |declared| declared.min(available));
        if frames == 0 {
            return Err(OpusError::NoSamples { pre_skip }.into());
        }
        Ok(Self {
            pre_skip,
            frames,
            emitted_end: emitted_end.unwrap_or(last_packet_end),
        })
    }

    pub(super) const fn granule_end(&self) -> u64 {
        self.frames + self.pre_skip as u64
    }
}

pub(super) enum WriteError<E> {
    Input(E),
    Output(io::Error),
}

impl WriteError<OpusError> {
    pub(super) fn into_audio(self) -> AudioError {
        match self {
            Self::Input(error) => AudioError::InvalidOpus(error),
            Self::Output(error) => AudioError::Write(error),
        }
    }
}

pub(super) fn write<S: PacketSource>(
    header: &StreamHeader,
    count: &PacketCount,
    source: &mut S,
    points: Option<Loop>,
    out: &mut (impl Write + ?Sized),
) -> Result<OggReport, WriteError<S::Error>> {
    let mut ogg = OggWriter::new(out, SERIAL);
    let mut packet = Vec::new();
    match &header.head {
        Some(head) => packet.extend_from_slice(head),
        None => header::head_packet(&header.layout, count.pre_skip, &mut packet),
    }
    ogg.write_packet(&packet, 0).map_err(WriteError::Output)?;
    ogg.end_page().map_err(WriteError::Output)?;
    packet.clear();
    let points = points.filter(|points| points.fits(count.frames));
    let loop_points = if let Some(tags) = &header.tags {
        packet.extend_from_slice(tags);
        points.filter(|&points| insert_comments(&mut packet, TAGS_MAGIC_LEN, points))
    } else {
        header::tags_packet(points, &mut packet);
        points
    };
    ogg.write_packet(&packet, 0).map_err(WriteError::Output)?;
    ogg.end_page().map_err(WriteError::Output)?;
    let end = count.granule_end();
    let mut granule = 0_u64;
    let mut index = 0;
    while granule < end {
        let packet = source
            .read_packet()
            .map_err(WriteError::Input)?
            .ok_or_else(|| {
                WriteError::Input(
                    OpusError::MissingSamples {
                        expected: count.frames,
                        decoded: granule.saturating_sub(u64::from(count.pre_skip)),
                    }
                    .into(),
                )
            })?;
        let samples = framing::packet_samples(packet).map_err(|reason| {
            WriteError::Input(OpusError::InvalidPacket { index, reason }.into())
        })?;
        granule = (granule + u64::from(samples)).min(end);
        ogg.write_packet(packet, granule)
            .map_err(WriteError::Output)?;
        index += 1;
    }
    let size = ogg.finish().map_err(WriteError::Output)?;
    Ok(OggReport {
        codec: Codec::Opus,
        channels: u16::try_from(header.layout.mapping.len()).unwrap_or(u16::MAX),
        sample_rate: OPUS_RATE,
        sample_count: count.frames,
        size,
        codebooks: None,
        loop_points,
    })
}
