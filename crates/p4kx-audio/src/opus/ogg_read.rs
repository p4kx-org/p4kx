// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::OpusError;
use super::framing::MAX_PACKET_LEN;
use crate::ogg::crc_update;

pub(super) const HEADER_LEN: usize = 27;
const CRC_RANGE_START: usize = 22;
const CRC_RANGE_END: usize = 26;
const FLAG_CONTINUED: u8 = 0x01;
const FLAG_LAST: u8 = 0x04;

pub(super) trait PageSource {
    type Error: From<OpusError>;

    fn length(&self) -> u64;

    fn prefix(&mut self, offset: u64) -> Result<[u8; HEADER_LEN], Self::Error>;

    fn load(
        &mut self,
        offset: u64,
        prefix: [u8; HEADER_LEN],
        segments: usize,
    ) -> Result<(), Self::Error>;

    fn page(&self) -> &[u8];
}

/// Checked packets from one logical Ogg stream, with one reusable assembly buffer.
pub(super) struct OggPackets<S> {
    source: S,
    next_page: u64,
    segments: usize,
    segment: usize,
    body_offset: usize,
    serial: Option<u32>,
    last_page: bool,
    packet: Vec<u8>,
    open: bool,
}

impl<'a> OggPackets<SlicePage<'a>> {
    pub(super) const fn new(data: &'a [u8]) -> Self {
        Self::with_source(SlicePage { data, page: &[] })
    }
}

impl<S: PageSource> OggPackets<S> {
    pub(super) const fn with_source(source: S) -> Self {
        Self {
            source,
            next_page: 0,
            segments: 0,
            segment: 0,
            body_offset: 0,
            serial: None,
            last_page: false,
            packet: Vec::new(),
            open: false,
        }
    }

    pub(super) const fn checked_end(&self) -> u64 {
        self.next_page
    }

    pub(super) fn next_packet(&mut self) -> Result<Option<&[u8]>, S::Error> {
        if !self.open {
            self.packet.clear();
        }
        loop {
            while self.segment < self.segments {
                let page = self.source.page();
                let value = page
                    .get(HEADER_LEN + self.segment)
                    .copied()
                    .ok_or(OpusError::InvalidOgg("a page's lacing is truncated"))?;
                let len = usize::from(value);
                let end = self.body_offset.saturating_add(len);
                let segment = page
                    .get(self.body_offset..end)
                    .ok_or(OpusError::InvalidOgg("a page body is truncated"))?;
                if self.packet.len() + len > MAX_PACKET_LEN {
                    return Err(OpusError::InvalidOgg("a packet exceeds 64 KiB").into());
                }
                self.packet.extend_from_slice(segment);
                self.segment += 1;
                self.body_offset = end;
                self.open = value == u8::MAX;
                if !self.open {
                    return Ok(Some(&self.packet));
                }
            }
            if self.last_page || self.next_page >= self.source.length() {
                return if self.open {
                    Err(OpusError::InvalidOgg("the last packet is unfinished").into())
                } else {
                    Ok(None)
                };
            }
            if let Err(error) = self.read_page() {
                self.next_page = self.source.length();
                self.segments = 0;
                self.open = false;
                return Err(error);
            }
        }
    }

    fn read_page(&mut self) -> Result<(), S::Error> {
        let prefix = self.source.prefix(self.next_page)?;
        if prefix.get(..5) != Some(b"OggS\0".as_slice()) {
            return Err(OpusError::InvalidOgg("a page lacks the OggS capture pattern").into());
        }
        let flags = prefix.get(5).copied().unwrap_or_default();
        let serial = word(&prefix, 14);
        if *self.serial.get_or_insert(serial) != serial {
            return Err(
                OpusError::InvalidOgg("the data holds more than one logical stream").into(),
            );
        }
        let segments = usize::from(prefix.last().copied().unwrap_or_default());
        self.source.load(self.next_page, prefix, segments)?;
        let page = self.source.page();
        let crc = [
            page.get(..CRC_RANGE_START).unwrap_or_default(),
            &[0; 4],
            page.get(CRC_RANGE_END..).unwrap_or_default(),
        ]
        .iter()
        .fold(0, |crc, part| crc_update(crc, part));
        if crc != word(&prefix, CRC_RANGE_START) {
            return Err(OpusError::InvalidOgg("a page's CRC does not match").into());
        }
        if (flags & FLAG_CONTINUED != 0) != self.open {
            return Err(OpusError::InvalidOgg("a page's continuation flag is wrong").into());
        }
        self.segments = segments;
        self.segment = 0;
        self.body_offset = HEADER_LEN + segments;
        self.last_page = flags & FLAG_LAST != 0;
        self.next_page += page.len() as u64;
        Ok(())
    }
}

fn word(prefix: &[u8], offset: usize) -> u32 {
    prefix
        .get(offset..)
        .and_then(<[u8]>::first_chunk::<4>)
        .map_or(0, |bytes| u32::from_le_bytes(*bytes))
}

pub(super) fn page_len(lacing: &[u8]) -> usize {
    HEADER_LEN
        + lacing.len()
        + lacing
            .iter()
            .map(|&value| usize::from(value))
            .sum::<usize>()
}

pub(super) struct SlicePage<'a> {
    data: &'a [u8],
    page: &'a [u8],
}

impl<'a> SlicePage<'a> {
    fn rest(&self, offset: u64) -> &'a [u8] {
        usize::try_from(offset)
            .ok()
            .and_then(|offset| self.data.get(offset..))
            .unwrap_or_default()
    }
}

impl PageSource for SlicePage<'_> {
    type Error = OpusError;

    fn length(&self) -> u64 {
        self.data.len() as u64
    }

    fn prefix(&mut self, offset: u64) -> Result<[u8; HEADER_LEN], OpusError> {
        self.rest(offset)
            .first_chunk::<HEADER_LEN>()
            .copied()
            .ok_or(OpusError::InvalidOgg("a page header is truncated"))
    }

    fn load(&mut self, offset: u64, _: [u8; HEADER_LEN], segments: usize) -> Result<(), OpusError> {
        let rest = self.rest(offset);
        let lacing = rest
            .get(HEADER_LEN..HEADER_LEN + segments)
            .ok_or(OpusError::InvalidOgg("a page's lacing is truncated"))?;
        self.page = rest
            .get(..page_len(lacing))
            .ok_or(OpusError::InvalidOgg("a page body is truncated"))?;
        Ok(())
    }

    fn page(&self) -> &[u8] {
        self.page
    }
}
