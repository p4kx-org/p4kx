// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;
use std::sync::Arc;

use crate::read_at::{ReadAt, SeekSource};
use crate::riff::{self, RiffDirectory, TailPolicy};
use crate::{AudioError, ByteOrder, WemCueText, WemReadError, WemSourceChunk};

const TYPE_LEN: u64 = 4;
const CUE_ID_LEN: u64 = 4;
const REGION_HEADER_LEN: usize = 20;

/// Text and original encoding declarations for a cue's data region.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WemCueTextRegion {
    /// Original cue identifier.
    pub cue_id: u32,
    /// Original length of the associated data region in samples.
    pub sample_length: u32,
    /// Original purpose word as its four stored bytes.
    pub purpose: [u8; 4],
    /// Original country code.
    pub country: u16,
    /// Original language code.
    pub language: u16,
    /// Original dialect code.
    pub dialect: u16,
    /// Original text code-page word.
    pub code_page: u16,
    /// Complete original text byte region.
    pub text_range: Range<u64>,
}

impl WemCueTextRegion {
    fn parse(order: ByteOrder, raw: &[u8], text_range: Range<u64>) -> Option<Self> {
        Some(Self {
            cue_id: order.u32_at(raw, 0)?,
            sample_length: order.u32_at(raw, 4)?,
            purpose: *raw.get(8..)?.first_chunk::<4>()?,
            country: order.u16_at(raw, 12)?,
            language: order.u16_at(raw, 14)?,
            dialect: order.u16_at(raw, 16)?,
            code_page: order.u16_at(raw, 18)?,
            text_range,
        })
    }
}

/// The interpretation of a bounded original LIST subchunk.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WemListPayload {
    /// A cue label in an associated-data list.
    Label(WemCueText),
    /// A cue comment in an associated-data list.
    Note(WemCueText),
    /// An associated text region with an original code page.
    TextRegion(WemCueTextRegion),
    /// A subchunk whose payload is retained through the original range.
    Opaque,
    /// The subchunk is bounded, but too short for its recognized fixed fields.
    Malformed {
        /// Original available body length.
        available: u64,
        /// Minimum body length for these fields.
        needed: u64,
    },
}

impl WemListPayload {
    const fn text_range(&self) -> Option<&Range<u64>> {
        match self {
            Self::Label(text) | Self::Note(text) => Some(&text.text_range),
            Self::TextRegion(text) => Some(&text.text_range),
            Self::Opaque | Self::Malformed { .. } => None,
        }
    }
}

/// A physical subchunk bound to its original WEM and LIST occurrence.
#[derive(Debug)]
pub struct WemListItem {
    chunk: WemSourceChunk,
    payload: WemListPayload,
    owner: Arc<()>,
    parent: u64,
}

impl WemListItem {
    /// Returns the original subchunk geometry and its ordinal within the LIST.
    #[must_use]
    pub const fn chunk(&self) -> WemSourceChunk {
        self.chunk
    }

    /// Returns the typed payload or the original preservation state.
    #[must_use]
    pub const fn payload(&self) -> &WemListPayload {
        &self.payload
    }
}

/// Fixed-memory reader of the immediate subchunks of one physical LIST occurrence.
pub struct WemList<'input, R> {
    input: &'input mut SeekSource<R>,
    directory: RiffDirectory,
    list_type: [u8; 4],
    order: ByteOrder,
    owner: Arc<()>,
    parent: u64,
    stopped: bool,
}

impl<'input, R: Read + Seek> WemList<'input, R> {
    pub(crate) fn open(
        input: &'input mut SeekSource<R>,
        order: ByteOrder,
        chunk: WemSourceChunk,
        owner: &Arc<()>,
    ) -> Result<Self, WemReadError> {
        let end = chunk.body_end_exclusive();
        let list_type = riff::read_within(input, chunk.body_offset, end)?;
        let directory = RiffDirectory::within(
            order,
            chunk.body_offset + TYPE_LEN,
            end,
            TailPolicy::Checked,
        );
        Ok(Self {
            input,
            directory,
            list_type,
            order,
            owner: Arc::clone(owner),
            parent: chunk.ordinal,
            stopped: false,
        })
    }

    /// Returns the original four-byte list type at the beginning of the LIST body.
    #[must_use]
    pub const fn list_type(&self) -> [u8; 4] {
        self.list_type
    }

    /// Copies the observed text range into a caller-sized buffer from the same LIST read.
    ///
    /// # Errors
    /// Returns an error for a foreign item, an item without text, a wrong buffer length or a read failure.
    pub fn read_text(&mut self, item: &WemListItem, buffer: &mut [u8]) -> Result<(), WemReadError> {
        if self.parent != item.parent || !Arc::ptr_eq(&self.owner, &item.owner) {
            return Err(WemReadError::InvalidOccurrence);
        }
        let range = item
            .payload
            .text_range()
            .ok_or(WemReadError::InvalidOccurrence)?;
        if buffer.len() as u64 != range.end - range.start {
            return Err(WemReadError::TextBufferLength);
        }
        self.input.read_at(range.start, buffer)
    }

    fn payload(&mut self, chunk: WemSourceChunk) -> Result<WemListPayload, WemReadError> {
        if &self.list_type != b"adtl" {
            return Ok(WemListPayload::Opaque);
        }
        match &chunk.id {
            b"labl" | b"note" => self.cue_text(chunk),
            b"ltxt" => self.region(chunk),
            _ => Ok(WemListPayload::Opaque),
        }
    }

    fn cue_text(&mut self, chunk: WemSourceChunk) -> Result<WemListPayload, WemReadError> {
        if chunk.body_len < CUE_ID_LEN {
            return Ok(short(chunk, CUE_ID_LEN));
        }
        let end = chunk.body_end_exclusive();
        let raw = riff::read_within(self.input, chunk.body_offset, end)?;
        let text = WemCueText::read(
            self.input,
            self.order.u32(raw),
            chunk.body_offset + CUE_ID_LEN..end,
        )?;
        if &chunk.id == b"labl" {
            Ok(WemListPayload::Label(text))
        } else {
            Ok(WemListPayload::Note(text))
        }
    }

    fn region(&mut self, chunk: WemSourceChunk) -> Result<WemListPayload, WemReadError> {
        if chunk.body_len < REGION_HEADER_LEN as u64 {
            return Ok(short(chunk, REGION_HEADER_LEN as u64));
        }
        let end = chunk.body_end_exclusive();
        let raw: [u8; REGION_HEADER_LEN] = riff::read_within(self.input, chunk.body_offset, end)?;
        let range = chunk.body_offset + REGION_HEADER_LEN as u64..end;
        let text = WemCueTextRegion::parse(self.order, &raw, range).ok_or(
            AudioError::InvalidFormat("the text region fields do not fit in the checked body"),
        )?;
        Ok(WemListPayload::TextRegion(text))
    }

    fn item(&mut self, chunk: WemSourceChunk) -> Result<WemListItem, WemReadError> {
        Ok(WemListItem {
            payload: self.payload(chunk)?,
            chunk,
            owner: Arc::clone(&self.owner),
            parent: self.parent,
        })
    }
}

impl<R: Read + Seek> Iterator for WemList<'_, R> {
    type Item = Result<WemListItem, WemReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }
        let result = self
            .directory
            .next(self.input)?
            .and_then(|chunk| self.item(chunk));
        if result.is_err() {
            self.stopped = true;
        }
        Some(result)
    }
}

const fn short(chunk: WemSourceChunk, needed: u64) -> WemListPayload {
    WemListPayload::Malformed {
        available: chunk.body_len,
        needed,
    }
}
