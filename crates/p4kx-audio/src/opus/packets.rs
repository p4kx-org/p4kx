// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::OpusError;
use super::framing::{self, MAX_PACKET_LEN};
use super::ogg_read::{OggPackets, SlicePage};
use super::stream::PacketSource;

pub(super) const PACKET_SIZE_LEN: usize = size_of::<u16>();
pub(super) const SWITCH_HEADER_LEN: usize = 8;

/// Where the packets of an Opus WEM come from.
#[derive(Clone, Copy)]
pub(crate) enum Framing<'a> {
    /// Wwise Opus: packets back to back, their sizes in the `seek` chunk as 16-bit values.
    Table(&'a [u8]),
    /// Switch Opus: each packet after an 8-byte header holding its big-endian size.
    Switch,
    /// A standard Ogg Opus stream, its two header packets first.
    Ogg,
}

/// The audio packets of an Opus WEM in order, each checked against the bounds of the data.
pub(crate) struct Packets<'a> {
    data: &'a [u8],
    framing: Framing<'a>,
    offset: usize,
    sizes: &'a [u8],
    ogg: Option<OggPackets<SlicePage<'a>>>,
    index: u64,
}

impl<'a> Packets<'a> {
    /// Starts at the first audio packet of `data`; for an Ogg stream the two header packets are
    /// skipped.
    pub(crate) fn new(data: &'a [u8], framing: Framing<'a>) -> Result<Self, OpusError> {
        let (sizes, ogg) = match framing {
            Framing::Table(sizes) => (sizes, None),
            Framing::Switch => (&[][..], None),
            Framing::Ogg => {
                let mut ogg = OggPackets::new(data);
                for _ in 0..2 {
                    ogg.next_packet()?
                        .ok_or(OpusError::InvalidOgg("the header packets are missing"))?;
                }
                (&[][..], Some(ogg))
            }
        };
        Ok(Self {
            data,
            framing,
            offset: 0,
            sizes,
            ogg,
            index: 0,
        })
    }

    /// Returns the next audio packet, or `None` after the last one.
    pub(crate) fn next_packet(&mut self) -> Option<Result<&[u8], OpusError>> {
        let index = self.index;
        let out_of_bounds = OpusError::PacketOutOfBounds { index };
        let packet = match self.framing {
            Framing::Ogg => match self.ogg.as_mut()?.next_packet() {
                Ok(packet) => packet?,
                Err(error) => return Some(Err(error)),
            },
            Framing::Table(_) => {
                let (size, rest) = self.sizes.split_first_chunk::<PACKET_SIZE_LEN>()?;
                self.sizes = rest;
                let len = usize::from(u16::from_le_bytes(*size));
                let packet = self
                    .offset
                    .checked_add(len)
                    .and_then(|end| self.data.get(self.offset..end));
                let Some(packet) = packet else {
                    self.sizes = &[];
                    return Some(Err(out_of_bounds));
                };
                self.offset += packet.len();
                packet
            }
            Framing::Switch => {
                let header = self.data.get(self.offset..)?;
                if header.is_empty() {
                    return None;
                }
                let start = self.offset.saturating_add(SWITCH_HEADER_LEN);
                let packet = switch_size(header, index)
                    .ok()
                    .and_then(|len| self.data.get(start..start.checked_add(len)?));
                let Some(packet) = packet else {
                    self.offset = self.data.len();
                    return Some(Err(out_of_bounds));
                };
                self.offset = start + packet.len();
                packet
            }
        };
        self.index += 1;
        Some(Ok(packet))
    }
}

impl PacketSource for Packets<'_> {
    type Error = OpusError;

    fn read_packet(&mut self) -> Result<Option<&[u8]>, OpusError> {
        self.next_packet().transpose()
    }

    fn checked_end(&self) -> u64 {
        self.ogg
            .as_ref()
            .map_or(self.offset as u64, OggPackets::checked_end)
    }
}

pub(super) fn switch_size(prefix: &[u8], index: u64) -> Result<usize, OpusError> {
    prefix
        .first_chunk::<4>()
        .and_then(|size| usize::try_from(u32::from_be_bytes(*size)).ok())
        .filter(|&len| len <= MAX_PACKET_LEN)
        .ok_or(OpusError::PacketOutOfBounds { index })
}

/// Checks a multistream packet of `streams` streams and returns its samples per channel.
pub(crate) fn multistream_samples(
    packet: &[u8],
    streams: u8,
    index: u64,
) -> Result<u32, OpusError> {
    let invalid = |reason| OpusError::InvalidPacket { index, reason };
    let mut rest = packet;
    let mut samples = None;
    for _ in 1..streams {
        let stream = framing::self_delimited(rest).map_err(invalid)?;
        if *samples.get_or_insert(stream.samples) != stream.samples {
            return Err(invalid("the streams differ in duration"));
        }
        rest = rest.get(stream.end..).unwrap_or_default();
    }
    let last = framing::packet_samples(rest).map_err(invalid)?;
    if *samples.get_or_insert(last) != last {
        return Err(invalid("the streams differ in duration"));
    }
    Ok(last)
}
