// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use crate::error::CryXmlError;

const INVALID_BYTE: char = '?';

/// The string data of a `CryXml` file: zero-terminated strings addressed by byte offset.
///
/// The data is validated as UTF-8 once, and the terminators are indexed once, so every lookup is
/// a binary search instead of a scan.
pub(crate) struct Strings<'a> {
    text: Cow<'a, str>,
    ends: Vec<u32>,
    repaired_utf8: bool,
}

/// The byte range of one string within the string data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    start: u32,
    end: u32,
}

impl Span {
    /// Returns the string this span covers in `text`.
    pub(crate) fn get(self, text: &str) -> &str {
        text.get(self.start as usize..self.end as usize)
            .unwrap_or_default()
    }

    pub(crate) const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

impl<'a> Strings<'a> {
    /// Indexes `data`, borrowing it when it is valid UTF-8 and otherwise copying it with every
    /// byte of an invalid sequence replaced by `?`, so that offsets keep their meaning.
    pub(crate) fn new(data: &'a [u8]) -> Self {
        let (text, repaired_utf8) = str::from_utf8(data).map_or_else(
            |_| (Cow::Owned(repair(data)), true),
            |text| (Cow::Borrowed(text), false),
        );
        let ends = data
            .iter()
            .enumerate()
            .filter(|&(_, &byte)| byte == 0)
            .filter_map(|(index, _)| u32::try_from(index).ok())
            .collect();
        Self {
            text,
            ends,
            repaired_utf8,
        }
    }

    /// Returns the span of the string at `offset`; a string that starts inside a multi-byte
    /// character begins at the next character.
    pub(crate) fn span(&self, offset: u32) -> Result<Span, CryXmlError> {
        let bad = CryXmlError::BadString { offset };
        if offset as usize >= self.text.len() {
            return Err(bad);
        }
        let terminator = self.ends.partition_point(|&end| end < offset);
        let end = *self.ends.get(terminator).ok_or(bad)?;
        let mut start = offset;
        while start < end && !self.text.is_char_boundary(start as usize) {
            start += 1;
        }
        Ok(Span { start, end })
    }

    pub(crate) const fn repaired_utf8(&self) -> bool {
        self.repaired_utf8
    }

    pub(crate) fn into_text(self) -> Cow<'a, str> {
        self.text
    }
}

fn repair(data: &[u8]) -> String {
    let mut text = String::with_capacity(data.len());
    for chunk in data.utf8_chunks() {
        text.push_str(chunk.valid());
        text.extend(std::iter::repeat_n(INVALID_BYTE, chunk.invalid().len()));
    }
    text
}
