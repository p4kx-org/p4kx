// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::{Filter, Peekable};
use std::ops::Range;

use super::VorbisError;
use super::bits::{BitWriter, ilog};
use super::header::Header;
use crate::ByteOrder;

pub(super) const PACKET_HEADER_LEN: usize = 2;

pub(super) fn payload_range(
    length: usize,
    offset: usize,
    size: u16,
) -> Result<Range<usize>, VorbisError> {
    let start = offset.checked_add(PACKET_HEADER_LEN);
    let range = start.and_then(|start| Some(start..start.checked_add(usize::from(size))?));
    range
        .filter(|range| range.end <= length)
        .ok_or(VorbisError::PacketOutOfBounds {
            offset: offset as u64,
        })
}

/// One Wwise packet: its offset in the `data` chunk and its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Packet<'a> {
    pub(crate) offset: usize,
    pub(crate) payload: &'a [u8],
}

/// The packets of a `data` chunk from an offset on, each a 16-bit size and its payload.
#[derive(Debug, Clone)]
pub(crate) struct Packets<'a> {
    data: &'a [u8],
    order: ByteOrder,
    offset: usize,
}

impl<'a> Packets<'a> {
    pub(crate) const fn new(data: &'a [u8], order: ByteOrder, offset: usize) -> Self {
        Self {
            data,
            order,
            offset,
        }
    }
}

impl<'a> Iterator for Packets<'a> {
    type Item = Result<Packet<'a>, VorbisError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.data.len() {
            return None;
        }
        let offset = self.offset;
        let payload = self.order.u16_at(self.data, offset).and_then(|size| {
            self.data
                .get(payload_range(self.data.len(), offset, size).ok()?)
        });
        let Some(payload) = payload else {
            self.offset = self.data.len();
            return Some(Err(VorbisError::PacketOutOfBounds {
                offset: offset as u64,
            }));
        };
        self.offset = offset + PACKET_HEADER_LEN + payload.len();
        Some(Ok(Packet { offset, payload }))
    }
}

type Item<'a> = Result<Packet<'a>, VorbisError>;
type NonEmpty<'a> = Peekable<Filter<Packets<'a>, fn(&Item<'a>) -> bool>>;

/// A rebuilt audio packet: its offset in the `data` chunk, its granule position and whether it
/// is the last one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rebuilt {
    pub(crate) offset: usize,
    pub(crate) granule: u64,
    pub(crate) last: bool,
}

/// Rebuilds the Wwise audio packets as Vorbis audio packets and counts their samples.
#[derive(Debug)]
pub(crate) struct AudioPackets<'a> {
    packets: NonEmpty<'a>,
    rebuild: PacketRebuild<'a>,
}

#[derive(Debug)]
pub(super) struct PacketRebuild<'a> {
    header: &'a Header,
    long_modes: &'a [bool],
    mode_bits: u32,
    previous: Option<bool>,
    granule: u64,
}

impl<'a> AudioPackets<'a> {
    pub(crate) fn new(
        data: &'a [u8],
        order: ByteOrder,
        header: &'a Header,
        long_modes: &'a [bool],
    ) -> Self {
        let not_empty: fn(&Item<'a>) -> bool =
            |item| !matches!(item, Ok(packet) if packet.payload.is_empty());
        Self {
            packets: Packets::new(data, order, header.audio_offset)
                .filter(not_empty)
                .peekable(),
            rebuild: PacketRebuild::new(header, long_modes),
        }
    }

    /// Rebuilds the next packet into `out`; `None` after the last one.
    pub(crate) fn next_into(&mut self, out: &mut Vec<u8>) -> Option<Result<Rebuilt, VorbisError>> {
        let packet = match self.packets.next()? {
            Ok(packet) => packet,
            Err(error) => return Some(Err(error)),
        };
        let next = self.packets.peek();
        let next_byte = next
            .and_then(|item| item.as_ref().ok())
            .and_then(|packet| packet.payload.first().copied());
        Some(self.rebuild.rebuild(packet, next_byte, next.is_none(), out))
    }
}

impl<'a> PacketRebuild<'a> {
    pub(super) fn new(header: &'a Header, long_modes: &'a [bool]) -> Self {
        let mode_count = u32::try_from(long_modes.len()).unwrap_or(u32::MAX);
        Self {
            header,
            long_modes,
            mode_bits: ilog(mode_count.saturating_sub(1)),
            previous: None,
            granule: 0,
        }
    }

    pub(super) fn rebuild(
        &mut self,
        packet: Packet<'_>,
        next_byte: Option<u8>,
        last: bool,
        out: &mut Vec<u8>,
    ) -> Result<Rebuilt, VorbisError> {
        let invalid = |reason| VorbisError::InvalidAudioPacket {
            offset: packet.offset as u64,
            reason,
        };
        let (&first, rest) = packet
            .payload
            .split_first()
            .ok_or_else(|| invalid("the packet is empty"))?;
        let mode_mask = (1_u8 << self.mode_bits) - 1;
        out.clear();
        let long = if self.header.modified_packets {
            let mode = first & mode_mask;
            let long = self
                .is_long(mode)
                .ok_or_else(|| invalid("the mode does not exist"))?;
            let next_long = next_byte
                .and_then(|byte| self.is_long(byte & mode_mask))
                .unwrap_or(false);
            let mut writer = BitWriter::new(out);
            writer.write(0, 1);
            writer.write(u32::from(mode), self.mode_bits);
            if long {
                writer.write(u32::from(self.previous.unwrap_or(false)), 1);
                writer.write(u32::from(next_long), 1);
            }
            writer.write(u32::from(first >> self.mode_bits), 8 - self.mode_bits);
            writer.write_bytes(rest);
            writer.finish();
            long
        } else {
            if first & 1 != 0 {
                return Err(invalid("a header packet stands among the audio packets"));
            }
            out.extend_from_slice(packet.payload);
            self.is_long((first >> 1) & mode_mask)
                .ok_or_else(|| invalid("the mode does not exist"))?
        };
        let block = self.header.block_size(long);
        if let Some(previous) = self.previous {
            let samples = self.header.block_size(previous) / 4 + block / 4;
            self.granule = self
                .granule
                .checked_add(samples)
                .ok_or_else(|| invalid("the granule position overflows"))?;
        }
        self.previous = Some(long);
        Ok(Rebuilt {
            offset: packet.offset,
            granule: self.granule,
            last,
        })
    }

    fn is_long(&self, mode: u8) -> Option<bool> {
        self.long_modes.get(usize::from(mode)).copied()
    }
}
