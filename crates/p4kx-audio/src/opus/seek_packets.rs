// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use super::OpusError;
use super::framing;
use super::header::{EmbeddedOpusHeaderPages, StreamHeader};
use super::ogg_read::OggPackets;
use super::packets::{self, PACKET_SIZE_LEN, SWITCH_HEADER_LEN};
use super::seek_pages::SeekPage;
use super::stream::PacketSource;
use crate::WemReadError;
use crate::read_at::SeekSource;

/// Original first-packet inputs used to infer the pre-skip of Switch Opus.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SwitchOpusPreSkip {
    /// WEM-relative packet length word that frames the original first packet.
    pub packet_size: Range<u64>,
    /// WEM-relative TOC byte and the following frame-count byte when the TOC requires it.
    pub duration_header: Range<u64>,
}

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
    pub(super) fn audio_packets<'a, R: Read + Seek>(
        &self,
        input: &'a mut SeekSource<R>,
    ) -> Result<SeekPackets<'a, R>, WemReadError> {
        let mut packets = self.packets(input);
        packets.skip_embedded_headers()?;
        Ok(packets)
    }

    pub(super) fn packets<'a, R: Read + Seek>(
        &self,
        input: &'a mut SeekSource<R>,
    ) -> SeekPackets<'a, R> {
        let framing = match &self.framing {
            Framing::Table(table) => Framed::Table(table.clone()),
            Framing::Switch => Framed::Switch,
            Framing::Ogg => {
                return SeekPackets::Ogg {
                    packets: OggPackets::with_source(SeekPage::new(input, self.data.clone())),
                    first_audio_page: None,
                };
            }
        };
        SeekPackets::Framed(FramedPackets {
            input,
            framing,
            start: self.data.start,
            offset: self.data.start,
            end: self.data.end,
            index: 0,
            switch_pre_skip: None,
            packet: Vec::new(),
        })
    }
}

pub(super) enum SeekPackets<'a, R> {
    Framed(FramedPackets<'a, R>),
    Ogg {
        packets: OggPackets<SeekPage<'a, R>>,
        first_audio_page: Option<u64>,
    },
}

impl<R: Read + Seek> SeekPackets<'_, R> {
    pub(super) fn switch_pre_skip(&self) -> Option<SwitchOpusPreSkip> {
        match self {
            Self::Framed(packets) => packets.switch_pre_skip.clone(),
            Self::Ogg { .. } => None,
        }
    }

    pub(super) const fn replayed_audio_start(&self) -> u64 {
        match self {
            Self::Ogg {
                first_audio_page: Some(start),
                ..
            } => *start,
            _ => 0,
        }
    }

    pub(super) fn replayed_seek_table(&self, selected: Option<&Range<u64>>) -> Option<Range<u64>> {
        if let Self::Framed(FramedPackets {
            framing: Framed::Table(table),
            ..
        }) = self
        {
            selected.map(|range| range.start..table.start)
        } else {
            None
        }
    }

    pub(super) fn embedded(
        &mut self,
        channels: u16,
        declared: Option<u64>,
    ) -> Result<(StreamHeader, EmbeddedOpusHeaderPages), WemReadError> {
        match self {
            Self::Ogg { packets, .. } => StreamHeader::embedded(packets, channels, declared),
            Self::Framed(_) => Err(OpusError::InvalidHeader("the input has no Ogg framing").into()),
        }
    }

    fn skip_embedded_headers(&mut self) -> Result<(), WemReadError> {
        if let Self::Ogg { packets, .. } = self {
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
            Self::Ogg {
                packets,
                first_audio_page,
            } => Ok(packets.next_packet_with_pages()?.map(|packet| {
                first_audio_page.get_or_insert(packet.pages.start);
                packet.bytes
            })),
        }
    }

    fn checked_end(&self) -> u64 {
        match self {
            Self::Framed(packets) => packets.offset - packets.start,
            Self::Ogg { packets, .. } => packets.checked_end(),
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
    switch_pre_skip: Option<SwitchOpusPreSkip>,
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
        if self.index == 0
            && matches!(self.framing, Framed::Switch)
            && let Some(&toc) = self.packet.first()
        {
            self.switch_pre_skip = Some(SwitchOpusPreSkip {
                packet_size: self.offset..self.offset + size_of::<u32>() as u64,
                duration_header: start..start + framing::duration_header_len(toc),
            });
        }
        self.offset = end;
        self.index += 1;
        Ok(Some(&self.packet))
    }
}
