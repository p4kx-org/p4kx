// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::AudioError;
use crate::read_at::{ReadAt, SliceSource};

const HEADER_LEN: u64 = 12;
const CHUNK_HEADER_LEN: u64 = 8;

/// The byte order of a WEM: little-endian `RIFF` or big-endian `RIFX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ByteOrder {
    /// A `RIFF` file.
    Little,
    /// A `RIFX` file, as written for big-endian consoles.
    Big,
}

impl ByteOrder {
    pub(crate) fn u16_at(self, bytes: &[u8], offset: usize) -> Option<u16> {
        let field = *bytes.get(offset..)?.first_chunk::<2>()?;
        Some(match self {
            Self::Little => u16::from_le_bytes(field),
            Self::Big => u16::from_be_bytes(field),
        })
    }

    pub(crate) fn u32_at(self, bytes: &[u8], offset: usize) -> Option<u32> {
        Some(self.u32(*bytes.get(offset..)?.first_chunk::<4>()?))
    }

    pub(crate) const fn u32(self, bytes: [u8; 4]) -> u32 {
        match self {
            Self::Little => u32::from_le_bytes(bytes),
            Self::Big => u32::from_be_bytes(bytes),
        }
    }
}

/// One original RIFF chunk occurrence, with coordinates relative to the WEM start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct WemSourceChunk {
    /// The original four bytes of the chunk tag.
    pub id: [u8; 4],
    /// Zero-based position in the RIFF directory.
    pub ordinal: u64,
    /// First byte of the eight-byte chunk header.
    pub header_offset: u64,
    /// First byte of the chunk body.
    pub body_offset: u64,
    /// Declared body length, excluding any padding.
    pub body_len: u64,
    /// First byte after the body and any admitted padding.
    pub next_offset: u64,
}

impl WemSourceChunk {
    /// Returns the exclusive end of the body, excluding any padding.
    #[must_use]
    pub const fn body_end_exclusive(&self) -> u64 {
        self.body_offset.saturating_add(self.body_len)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RiffHeader {
    pub(crate) order: ByteOrder,
    pub(crate) end: u64,
}

impl RiffHeader {
    pub(crate) fn read<I: ReadAt>(input: &mut I) -> Result<Self, I::Error> {
        let len = input.byte_length();
        let mut signature = [0; 4];
        if len < signature.len() as u64 {
            return Err(truncated(len, HEADER_LEN).into());
        }
        input.read_at(0, &mut signature)?;
        let order = match &signature {
            b"RIFF" => ByteOrder::Little,
            b"RIFX" => ByteOrder::Big,
            b"PLUG" => return Err(AudioError::PluginMedia.into()),
            b"MThd" => return Err(AudioError::MidiMedia.into()),
            _ => return Err(AudioError::NotWem.into()),
        };
        if len < HEADER_LEN {
            return Err(truncated(len, HEADER_LEN).into());
        }
        let mut remainder = [0; 8];
        input.read_at(4, &mut remainder)?;
        let [a, b, c, d, form_a, form_b, form_c, form_d] = remainder;
        match &[form_a, form_b, form_c, form_d] {
            b"WAVE" => {}
            b"RMID" if order == ByteOrder::Little => return Err(AudioError::MidiMedia.into()),
            _ => return Err(AudioError::NotWem.into()),
        }
        let end = u64::from(order.u32([a, b, c, d])) + CHUNK_HEADER_LEN;
        if end > len {
            return Err(truncated(len, end).into());
        }
        Ok(Self { order, end })
    }

    pub(crate) const fn directory(self, tail: TailPolicy) -> RiffDirectory {
        RiffDirectory::within(self.order, HEADER_LEN, self.end, tail)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum TailPolicy {
    Legacy,
    Checked,
}

pub(crate) struct RiffDirectory {
    order: ByteOrder,
    end: u64,
    next_offset: u64,
    ordinal: u64,
    tail: TailPolicy,
    stopped: bool,
}

impl RiffDirectory {
    pub(crate) const fn within(order: ByteOrder, start: u64, end: u64, tail: TailPolicy) -> Self {
        Self {
            order,
            end,
            next_offset: start,
            ordinal: 0,
            tail,
            stopped: false,
        }
    }

    pub(crate) fn next<I: ReadAt>(
        &mut self,
        input: &mut I,
    ) -> Option<Result<WemSourceChunk, I::Error>> {
        if self.stopped {
            return None;
        }
        let result = self.advance(input);
        if !matches!(result, Ok(Some(_))) {
            self.stopped = true;
        }
        result.transpose()
    }

    fn advance<I: ReadAt>(&mut self, input: &mut I) -> Result<Option<WemSourceChunk>, I::Error> {
        let header_offset = self.next_offset;
        let body_offset = header_offset + CHUNK_HEADER_LEN;
        if body_offset > self.end {
            if matches!(self.tail, TailPolicy::Checked) && header_offset != self.end {
                let needed = if header_offset > self.end {
                    header_offset
                } else {
                    body_offset
                };
                return Err(truncated(self.end, needed).into());
            }
            return Ok(None);
        }
        let (id, size) = chunk_header(input, self.order, header_offset)?;
        let body_len = u64::from(size);
        let body_end = body_offset + body_len;
        if body_end > self.end {
            let available = match self.tail {
                TailPolicy::Legacy => input.byte_length(),
                TailPolicy::Checked => self.end,
            };
            return Err(truncated(available, body_end).into());
        }
        let next_offset = if size % 2 == 1 {
            self.after_odd_chunk(input, body_end)?
        } else {
            body_end
        };
        let ordinal = self.ordinal;
        self.next_offset = next_offset;
        self.ordinal += 1;
        Ok(Some(WemSourceChunk {
            id,
            ordinal,
            header_offset,
            body_offset,
            body_len,
            next_offset,
        }))
    }

    // A valid header at the body end establishes the Wwise unpadded layout.
    fn after_odd_chunk<I: ReadAt>(&self, input: &mut I, body_end: u64) -> Result<u64, I::Error> {
        if body_end + CHUNK_HEADER_LEN <= self.end {
            let (id, size) = chunk_header(input, self.order, body_end)?;
            let complete = body_end + CHUNK_HEADER_LEN + u64::from(size) <= self.end;
            if complete && id.iter().all(|byte| (0x20..0x7F).contains(byte)) {
                return Ok(body_end);
            }
        }
        Ok((body_end + 1).min(self.end))
    }
}

pub(crate) fn read_within<const N: usize, I: ReadAt>(
    input: &mut I,
    offset: u64,
    bound: u64,
) -> Result<[u8; N], I::Error> {
    let needed = offset + N as u64;
    if needed > bound {
        return Err(truncated(bound, needed).into());
    }
    let mut bytes = [0; N];
    input.read_at(offset, &mut bytes)?;
    Ok(bytes)
}

fn chunk_header<I: ReadAt>(
    input: &mut I,
    order: ByteOrder,
    offset: u64,
) -> Result<([u8; 4], u32), I::Error> {
    let mut raw = [0; 8];
    input.read_at(offset, &mut raw)?;
    let [
        tag_first,
        tag_second,
        tag_third,
        tag_fourth,
        size_first,
        size_second,
        size_third,
        size_fourth,
    ] = raw;
    let tag = [tag_first, tag_second, tag_third, tag_fourth];
    let size = order.u32([size_first, size_second, size_third, size_fourth]);
    Ok((tag, size))
}

pub(crate) struct Chunk {
    pub(crate) id: [u8; 4],
    pub(crate) body: Range<usize>,
}

/// Slice adapter for the sound and packet decoders.
pub(crate) struct Chunks<'bytes> {
    input: SliceSource<'bytes>,
    directory: RiffDirectory,
}

impl<'bytes> Chunks<'bytes> {
    pub(crate) fn of(bytes: &'bytes [u8]) -> Result<(ByteOrder, Self), AudioError> {
        let mut input = SliceSource(bytes);
        let header = RiffHeader::read(&mut input)?;
        Ok((
            header.order,
            Self {
                input,
                directory: header.directory(TailPolicy::Legacy),
            },
        ))
    }
}

impl Iterator for Chunks<'_> {
    type Item = Result<Chunk, AudioError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.directory.next(&mut self.input).map(|result| {
            let chunk = result?;
            let body = slice_range(chunk)?;
            Ok(Chunk { id: chunk.id, body })
        })
    }
}

pub(crate) fn slice_range(chunk: WemSourceChunk) -> Result<Range<usize>, AudioError> {
    let end = chunk.body_end_exclusive();
    let start =
        usize::try_from(chunk.body_offset).map_err(|_| truncated(usize::MAX as u64, end))?;
    let end = usize::try_from(end).map_err(|_| truncated(usize::MAX as u64, end))?;
    Ok(start..end)
}

const fn truncated(len: u64, needed: u64) -> AudioError {
    AudioError::Truncated { len, needed }
}
