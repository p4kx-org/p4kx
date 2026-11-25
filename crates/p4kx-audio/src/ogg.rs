// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use crate::OggReport;

const PAGE_TARGET: usize = 4096;
const MAX_SEGMENTS: usize = 255;
const SEGMENT: usize = 255;
const HEADER_LEN: usize = 27;
const FLAG_CONTINUED: u8 = 0x01;
const FLAG_FIRST: u8 = 0x02;
const FLAG_LAST: u8 = 0x04;
const NO_GRANULE: u64 = u64::MAX;
const CRC_TABLE: [u32; 256] = crc_table();

/// Complete Ogg page groups, relative to the beginning of the converted stream.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct OggOutputRanges {
    /// Pages containing the identification packet.
    pub identification: Range<u64>,
    /// Pages containing the comment packet and any setup packet.
    pub comments_and_setup: Range<u64>,
    /// Audio pages including the end-of-stream page.
    pub audio: Range<u64>,
}

pub(crate) struct WrittenOgg {
    pub(crate) report: OggReport,
    pub(crate) output: OggOutputRanges,
}

impl WrittenOgg {
    pub(crate) const fn new(report: OggReport, identification_end: u64, header_end: u64) -> Self {
        Self {
            output: OggOutputRanges {
                identification: 0..identification_end,
                comments_and_setup: identification_end..header_end,
                audio: header_end..report.size,
            },
            report,
        }
    }
}

/// Writes packets of one logical bitstream as Ogg pages: pages of about 4 KiB, packets split
/// across pages where needed, the first page marked BOS and the last EOS.
///
/// The writer reuses its page buffer, so writing a packet allocates nothing once the buffer has
/// grown to the largest page.
#[derive(Debug)]
pub struct OggWriter<W> {
    out: W,
    serial: u32,
    sequence: u32,
    lacing: [u8; MAX_SEGMENTS],
    segments: usize,
    body: Vec<u8>,
    granule: Option<u64>,
    last_granule: u64,
    continued: bool,
    first: bool,
    written: u64,
}

impl<W: Write> OggWriter<W> {
    /// Creates a writer of the stream with the serial number `serial`.
    pub fn new(out: W, serial: u32) -> Self {
        Self {
            out,
            serial,
            sequence: 0,
            lacing: [0; MAX_SEGMENTS],
            segments: 0,
            body: Vec::with_capacity(PAGE_TARGET + SEGMENT),
            granule: None,
            last_granule: 0,
            continued: false,
            first: true,
            written: 0,
        }
    }

    /// Adds a packet whose last sample has the granule position `granule`; a page that is full
    /// is written first.
    ///
    /// # Errors
    ///
    /// Returns the error of the underlying writer.
    pub fn write_packet(&mut self, packet: &[u8], granule: u64) -> io::Result<()> {
        if self.body.len() >= PAGE_TARGET || self.segments == MAX_SEGMENTS {
            self.write_page(false)?;
        }
        let mut rest = packet;
        loop {
            while self.segments < MAX_SEGMENTS
                && let Some((segment, tail)) = rest.split_at_checked(SEGMENT)
            {
                self.push_segment(segment);
                rest = tail;
            }
            if self.segments < MAX_SEGMENTS {
                self.push_segment(rest);
                self.granule = Some(granule);
                self.last_granule = granule;
                return Ok(());
            }
            self.write_page(false)?;
            self.continued = true;
        }
    }

    /// Writes the pending page, so the next packet starts on a fresh page.
    ///
    /// # Errors
    ///
    /// Returns the error of the underlying writer.
    pub fn end_page(&mut self) -> io::Result<()> {
        if self.segments > 0 {
            self.write_page(false)?;
        }
        Ok(())
    }

    /// Writes the pending page as the last page of the stream and returns the bytes written in
    /// total.
    ///
    /// # Errors
    ///
    /// Returns the error of the underlying writer.
    pub fn finish(mut self) -> io::Result<u64> {
        self.write_page(true)?;
        self.out.flush()?;
        Ok(self.written)
    }

    pub(crate) const fn written_len(&self) -> u64 {
        self.written
    }

    fn push_segment(&mut self, segment: &[u8]) {
        if let Some(value) = self.lacing.get_mut(self.segments) {
            *value = u8::try_from(segment.len()).unwrap_or(u8::MAX);
            self.segments += 1;
            self.body.extend_from_slice(segment);
        }
    }

    fn write_page(&mut self, last: bool) -> io::Result<()> {
        let granule = match self.granule {
            Some(granule) => granule,
            None if last => self.last_granule,
            None => NO_GRANULE,
        };
        let mut flags = 0;
        if self.continued {
            flags |= FLAG_CONTINUED;
        }
        if self.first {
            flags |= FLAG_FIRST;
        }
        if last {
            flags |= FLAG_LAST;
        }
        let lacing = self.lacing.get(..self.segments).unwrap_or_default();
        let mut header = [0; HEADER_LEN];
        header[..4].copy_from_slice(b"OggS");
        header[5] = flags;
        header[6..14].copy_from_slice(&granule.to_le_bytes());
        header[14..18].copy_from_slice(&self.serial.to_le_bytes());
        header[18..22].copy_from_slice(&self.sequence.to_le_bytes());
        header[26] = u8::try_from(self.segments).unwrap_or(u8::MAX);
        let crc = [header.as_slice(), lacing, &self.body]
            .iter()
            .fold(0, |crc, part| crc_update(crc, part));
        header[22..26].copy_from_slice(&crc.to_le_bytes());
        self.out.write_all(&header)?;
        self.out.write_all(lacing)?;
        self.out.write_all(&self.body)?;
        self.written += (HEADER_LEN + lacing.len() + self.body.len()) as u64;
        self.sequence = self.sequence.wrapping_add(1);
        self.segments = 0;
        self.body.clear();
        self.granule = None;
        self.continued = false;
        self.first = false;
        Ok(())
    }
}

pub(crate) fn crc_update(crc: u32, bytes: &[u8]) -> u32 {
    bytes.iter().fold(crc, |crc, &byte| {
        let index = usize::from(((crc >> 24) as u8) ^ byte);
        (crc << 8) ^ CRC_TABLE.get(index).copied().unwrap_or_default()
    })
}

#[expect(
    clippy::indexing_slicing,
    reason = "the index stays below the table length"
)]
const fn crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut index = 0_u32;
    while index < 256 {
        let mut crc = index << 24;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 0x8000_0000 == 0 {
                crc << 1
            } else {
                (crc << 1) ^ 0x04C1_1DB7
            };
            bit += 1;
        }
        table[index as usize] = crc;
        index += 1;
    }
    table
}
