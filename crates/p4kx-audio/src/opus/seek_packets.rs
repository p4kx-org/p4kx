// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use super::OpusError;
use super::header::StreamHeader;
use super::ogg_read::OggPackets;
use super::packets::{self, PACKET_SIZE_LEN, SWITCH_HEADER_LEN};
use super::seek_pages::SeekPage;
use super::stream::PacketSource;
use crate::WemReadError;
use crate::read_at::SeekSource;

pub(super) struct Binding {
    pub(super) data: Range<u64>,
    pub(super) framing: Framing,
}

pub(super) enum Framing {
    Table(Range<u64>),
    Switch,
    Ogg,
}

impl Binding {
    pub(super) fn packets<'a, R: Read + Seek>(
        &self,
        input: &'a mut SeekSource<R>,
    ) -> SeekPackets<'a, R> {
        let framing = match &self.framing {
            Framing::Table(table) => Framed::Table(table.clone()),
            Framing::Switch => Framed::Switch,
            Framing::Ogg => {
                return SeekPackets::Ogg(OggPackets::with_source(SeekPage::new(
                    input,
                    self.data.clone(),
                )));
            }
        };
        SeekPackets::Framed(FramedPackets {
            input,
            framing,
            start: self.data.start,
            offset: self.data.start,
            end: self.data.end,
            index: 0,
            packet: Vec::new(),
        })
    }
}

pub(super) enum SeekPackets<'a, R> {
    Framed(FramedPackets<'a, R>),
    Ogg(OggPackets<SeekPage<'a, R>>),
}

impl<R: Read + Seek> SeekPackets<'_, R> {
    pub(super) fn embedded(
        &mut self,
        channels: u16,
        declared: Option<u64>,
    ) -> Result<StreamHeader, WemReadError> {
        match self {
            Self::Ogg(packets) => StreamHeader::embedded(packets, channels, declared),
            Self::Framed(_) => Err(OpusError::InvalidHeader("the input has no Ogg framing").into()),
        }
    }

    pub(super) fn skip_embedded_headers(&mut self) -> Result<(), WemReadError> {
        if let Self::Ogg(packets) = self {
            for _ in 0..2 {
                packets
                    .next_packet()?
                    .ok_or(OpusError::InvalidOgg("the header packets are missing"))?;
            }
        }
        Ok(())
    }
}

impl<R: Read + Seek> PacketSource for SeekPackets<'_, R> {
    type Error = WemReadError;

    fn read_packet(&mut self) -> Result<Option<&[u8]>, WemReadError> {
        match self {
            Self::Framed(packets) => packets.read_packet(),
            Self::Ogg(packets) => packets.next_packet(),
        }
    }

    fn checked_end(&self) -> u64 {
        match self {
            Self::Framed(packets) => packets.offset - packets.start,
            Self::Ogg(packets) => packets.checked_end(),
        }
    }
}

enum Framed {
    Table(Range<u64>),
    Switch,
}

pub(super) struct FramedPackets<'a, R> {
    input: &'a mut SeekSource<R>,
    framing: Framed,
    start: u64,
    offset: u64,
    end: u64,
    index: u64,
    packet: Vec<u8>,
}

impl<R: Read + Seek> FramedPackets<'_, R> {
    fn read_packet(&mut self) -> Result<Option<&[u8]>, WemReadError> {
        let out = OpusError::PacketOutOfBounds { index: self.index };
        let (start, length) = match &mut self.framing {
            Framed::Table(table) => {
                if table.is_empty() {
                    return Ok(None);
                }
                let mut size = [0; PACKET_SIZE_LEN];
                self.input.read_io_at(table.start, &mut size)?;
                table.start += PACKET_SIZE_LEN as u64;
                (self.offset, usize::from(u16::from_le_bytes(size)))
            }
            Framed::Switch => {
                if self.offset == self.end {
                    return Ok(None);
                }
                let start = self.offset + SWITCH_HEADER_LEN as u64;
                if start > self.end {
                    return Err(out.into());
                }
                let mut size = [0; SWITCH_HEADER_LEN];
                self.input.read_io_at(self.offset, &mut size)?;
                (start, packets::switch_size(&size, self.index)?)
            }
        };
        let end = start
            .checked_add(length as u64)
            .filter(|&end| end <= self.end)
            .ok_or(out)?;
        self.packet.resize(length, 0);
        self.input.read_io_at(start, &mut self.packet)?;
        self.offset = end;
        self.index += 1;
        Ok(Some(&self.packet))
    }
}
