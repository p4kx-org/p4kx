// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek};
use std::ops::Range;

use super::VorbisError;
use super::header::Header;
use super::packets::{PACKET_HEADER_LEN, Packet, PacketRebuild, Rebuilt, payload_range};
use super::setup::Setup;
use super::stream::{AudioRead, AudioSource, StreamError};
use crate::ByteOrder;
use crate::read_at::SeekSource;

pub(super) struct SeekAudio<'source, R> {
    pub(super) input: &'source mut SeekSource<R>,
    pub(super) data: Range<u64>,
    pub(super) order: ByteOrder,
}

impl<R: Read + Seek> SeekAudio<'_, R> {
    pub(super) fn prepare(
        &mut self,
        header: &Header,
        setup: &mut Vec<u8>,
    ) -> Result<Range<u64>, StreamError<io::Error>> {
        let mut packets = self.packets(header.setup_offset);
        let Some(range) = packets.next_range()? else {
            return Err(out_of_bounds(header.setup_offset).into());
        };
        packets.read(&range, setup)?;
        let setup = packets.absolute(range.start - PACKET_HEADER_LEN..range.end);
        let mut packets = self.packets(header.audio_offset);
        if header.audio_offset > packets.length() {
            return Err(out_of_bounds(header.audio_offset).into());
        }
        while packets.next_range()?.is_some() {}
        Ok(setup)
    }

    fn packets(&mut self, offset: usize) -> SeekPackets<'_, R> {
        SeekPackets {
            input: self.input,
            data: self.data.clone(),
            order: self.order,
            offset,
        }
    }
}

impl<R: Read + Seek> AudioSource for SeekAudio<'_, R> {
    type InputError = io::Error;

    fn audio<'a>(
        &'a mut self,
        header: &'a Header,
        setup: &'a Setup,
    ) -> impl AudioRead<Self::InputError> + 'a {
        SeekRebuilt {
            packets: self.packets(header.audio_offset),
            rebuild: PacketRebuild::new(header, &setup.long_modes),
            current: Vec::new(),
            next: Vec::new(),
            next_offset: None,
            started: false,
        }
    }
}

struct SeekPackets<'source, R> {
    input: &'source mut SeekSource<R>,
    data: Range<u64>,
    order: ByteOrder,
    offset: usize,
}

impl<R: Read + Seek> SeekPackets<'_, R> {
    fn length(&self) -> usize {
        usize::try_from(self.data.end - self.data.start).unwrap_or(usize::MAX)
    }

    const fn absolute(&self, range: Range<usize>) -> Range<u64> {
        self.data.start + range.start as u64..self.data.start + range.end as u64
    }

    fn next_range(&mut self) -> Result<Option<Range<usize>>, StreamError<io::Error>> {
        let length = self.length();
        let offset = self.offset;
        if offset >= length {
            return Ok(None);
        }
        let mut prefix = [0; PACKET_HEADER_LEN];
        if length - offset < prefix.len() {
            return Err(out_of_bounds(offset).into());
        }
        self.input
            .read_io_at(self.data.start + offset as u64, &mut prefix)
            .map_err(StreamError::Input)?;
        let size = self.order.u16_at(&prefix, 0).unwrap_or_default();
        let range = payload_range(length, offset, size)?;
        self.offset = range.end;
        Ok(Some(range))
    }

    fn read(
        &mut self,
        range: &Range<usize>,
        output: &mut Vec<u8>,
    ) -> Result<(), StreamError<io::Error>> {
        output.resize(range.len(), 0);
        self.input
            .read_io_at(self.data.start + range.start as u64, output)
            .map_err(StreamError::Input)
    }

    fn next_nonempty(
        &mut self,
        output: &mut Vec<u8>,
    ) -> Result<Option<usize>, StreamError<io::Error>> {
        while let Some(range) = self.next_range()? {
            if !range.is_empty() {
                self.read(&range, output)?;
                return Ok(Some(range.start - PACKET_HEADER_LEN));
            }
        }
        output.clear();
        Ok(None)
    }
}

struct SeekRebuilt<'a, R> {
    packets: SeekPackets<'a, R>,
    rebuild: PacketRebuild<'a>,
    current: Vec<u8>,
    next: Vec<u8>,
    next_offset: Option<usize>,
    started: bool,
}

impl<R: Read + Seek> AudioRead<io::Error> for SeekRebuilt<'_, R> {
    fn next_packet(
        &mut self,
        output: &mut Vec<u8>,
    ) -> Result<Option<Rebuilt>, StreamError<io::Error>> {
        let current_offset = if self.started {
            std::mem::swap(&mut self.current, &mut self.next);
            self.next_offset.take()
        } else {
            self.started = true;
            self.packets.next_nonempty(&mut self.current)?
        };
        let Some(offset) = current_offset else {
            return Ok(None);
        };
        self.next_offset = self.packets.next_nonempty(&mut self.next)?;
        let current = Packet {
            offset,
            payload: &self.current,
        };
        let rebuilt = self.rebuild.rebuild(
            current,
            self.next.first().copied(),
            self.next_offset.is_none(),
            output,
        )?;
        Ok(Some(rebuilt))
    }
}

pub(super) const fn out_of_bounds(offset: usize) -> VorbisError {
    VorbisError::PacketOutOfBounds {
        offset: offset as u64,
    }
}
