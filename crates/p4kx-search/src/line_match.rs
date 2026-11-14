// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::line_excerpt::LineExcerpt;
use crate::needle_scan::NeedleScan;
use crate::{Excerpt, Needle};

/// Why a complete source line cannot be represented as searchable text.
#[derive(Clone, Copy)]
pub(crate) enum LineError {
    Binary,
    MissingMatchRange,
}

/// Validates every UTF-8 byte while retaining bounded literal and excerpt state.
pub(crate) struct LineMatch<'needle> {
    scanner: NeedleScan<'needle>,
    excerpt: LineExcerpt,
    pending: [u8; 4],
    pending_len: usize,
    offset: u64,
}

impl<'needle> LineMatch<'needle> {
    pub(crate) fn new(needle: &'needle Needle) -> Self {
        Self {
            scanner: needle.scanner(),
            excerpt: LineExcerpt::new(needle.folded_len()),
            pending: [0; 4],
            pending_len: 0,
            offset: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.scanner.reset();
        self.excerpt.reset();
        self.pending_len = 0;
        self.offset = 0;
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) -> Result<(), LineError> {
        let bytes = self.continue_scalar(bytes)?;
        if self.pending_len != 0 || bytes.is_empty() {
            return Ok(());
        }
        match std::str::from_utf8(bytes) {
            Ok(text) => self.text(text),
            Err(error) => {
                let valid = bytes.get(..error.valid_up_to()).ok_or(LineError::Binary)?;
                self.text(std::str::from_utf8(valid).map_err(|_| LineError::Binary)?)?;
                if error.error_len().is_some() {
                    return Err(LineError::Binary);
                }
                let tail = bytes.get(error.valid_up_to()..).ok_or(LineError::Binary)?;
                self.pending
                    .get_mut(..tail.len())
                    .ok_or(LineError::Binary)?
                    .copy_from_slice(tail);
                self.pending_len = tail.len();
                Ok(())
            }
        }
    }

    fn continue_scalar<'bytes>(
        &mut self,
        mut bytes: &'bytes [u8],
    ) -> Result<&'bytes [u8], LineError> {
        while self.pending_len > 0 && !bytes.is_empty() {
            let (&byte, rest) = bytes.split_first().ok_or(LineError::Binary)?;
            *self
                .pending
                .get_mut(self.pending_len)
                .ok_or(LineError::Binary)? = byte;
            self.pending_len += 1;
            bytes = rest;
            let pending = self
                .pending
                .get(..self.pending_len)
                .ok_or(LineError::Binary)?;
            match std::str::from_utf8(pending) {
                Ok(text) => {
                    let character = text.chars().next().ok_or(LineError::Binary)?;
                    self.character(character)?;
                    self.pending_len = 0;
                }
                Err(error) if error.error_len().is_some() => return Err(LineError::Binary),
                Err(_) => {}
            }
        }
        Ok(bytes)
    }

    fn text(&mut self, text: &str) -> Result<(), LineError> {
        for character in text.chars() {
            self.character(character)?;
        }
        Ok(())
    }

    fn character(&mut self, character: char) -> Result<(), LineError> {
        let matched = self.excerpt.has_match();
        self.excerpt.push(character, self.offset);
        if !matched && let Some(range) = self.scanner.push(character, self.offset) {
            self.excerpt
                .capture(&range)
                .ok_or(LineError::MissingMatchRange)?;
        }
        self.offset += character.len_utf8() as u64;
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> Result<Option<Excerpt>, LineError> {
        if self.pending_len != 0 {
            return Err(LineError::Binary);
        }
        Ok(self.excerpt.take())
    }

    pub(crate) const fn memory(needle_chars: usize) -> u64 {
        NeedleScan::memory(needle_chars).saturating_add(LineExcerpt::memory(needle_chars))
    }
}
