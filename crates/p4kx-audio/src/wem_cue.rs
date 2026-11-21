// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::read_at::SeekSource;
use crate::riff;
use crate::{AudioError, ByteOrder, WemReadError, WemSourceChunk};

const COUNT_LEN: u64 = 4;
const POINT_LEN: usize = 24;

/// One original cue point in an observed WEM cue chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct WemCuePoint {
    /// Zero-based position within the containing cue chunk.
    pub ordinal: u64,
    /// Source-relative position of the first of six consecutive four-byte fields.
    pub offset: u64,
    /// Original cue identifier.
    pub id: u32,
    /// Original play-order position word.
    pub position: u32,
    /// Original `FourCC` of the referenced data chunk.
    pub target_chunk: [u8; 4],
    /// Original chunk-start word.
    pub chunk_start: u32,
    /// Original block-start word.
    pub block_start: u32,
    /// Original sample-offset word.
    pub sample_offset: u32,
}

/// Fixed-memory iteration over an observed physical cue chunk.
pub struct WemCue<'input, R> {
    input: &'input mut SeekSource<R>,
    order: ByteOrder,
    bound: u64,
    position: u64,
    count: u32,
    ordinal: u32,
    stopped: bool,
}

impl<'input, R: Read + Seek> WemCue<'input, R> {
    pub(crate) fn open(
        input: &'input mut SeekSource<R>,
        order: ByteOrder,
        chunk: WemSourceChunk,
    ) -> Result<Self, WemReadError> {
        let bound = chunk.body_end_exclusive();
        let raw = riff::read_within(input, chunk.body_offset, bound)?;
        Ok(Self {
            input,
            order,
            bound,
            position: chunk.body_offset + COUNT_LEN,
            count: order.u32(raw),
            ordinal: 0,
            stopped: false,
        })
    }

    fn point(&mut self) -> Result<WemCuePoint, WemReadError> {
        let raw: [u8; POINT_LEN] = riff::read_within(self.input, self.position, self.bound)?;
        let point = self.decode(&raw).ok_or(AudioError::InvalidFormat(
            "the cue point fields do not fit in the checked record",
        ))?;
        self.position += POINT_LEN as u64;
        self.ordinal += 1;
        Ok(point)
    }
}

impl<R> WemCue<'_, R> {
    /// Returns the original count at the start of the cue chunk.
    #[must_use]
    pub const fn point_count(&self) -> u32 {
        self.count
    }

    /// Returns the original residual bytes after all declared cue points were read.
    #[must_use]
    pub const fn trailing_range(&self) -> Option<Range<u64>> {
        if self.stopped || self.ordinal != self.count {
            None
        } else {
            Some(self.position..self.bound)
        }
    }

    fn decode(&self, raw: &[u8]) -> Option<WemCuePoint> {
        Some(WemCuePoint {
            ordinal: u64::from(self.ordinal),
            offset: self.position,
            id: self.order.u32_at(raw, 0)?,
            position: self.order.u32_at(raw, 4)?,
            target_chunk: *raw.get(8..)?.first_chunk::<4>()?,
            chunk_start: self.order.u32_at(raw, 12)?,
            block_start: self.order.u32_at(raw, 16)?,
            sample_offset: self.order.u32_at(raw, 20)?,
        })
    }
}

impl<R: Read + Seek> Iterator for WemCue<'_, R> {
    type Item = Result<WemCuePoint, WemReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped || self.ordinal == self.count {
            return None;
        }
        let result = self.point();
        if result.is_err() {
            self.stopped = true;
        }
        Some(result)
    }
}
