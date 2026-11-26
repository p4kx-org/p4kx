// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceEvidenceError;

use super::EntryPreviewProvenance;
use crate::PreviewTab;

/// The number of bytes in one row of the hex view.
pub(crate) const HEX_ROW_LEN: u8 = 16;

const PLACEHOLDER: char = '\u{B7}';

/// The data read from the start of an Entry, with its text and its line breaks.
#[derive(Debug)]
pub struct PreviewContents {
    bytes: Vec<u8>,
    size: u64,
    text: String,
    lines: Vec<Range<usize>>,
    provenance: Result<EntryPreviewProvenance, SourceEvidenceError>,
}

impl PreviewContents {
    /// Decodes `bytes`, the start of an Entry of `size` bytes.
    pub(crate) fn new(
        bytes: Vec<u8>,
        size: u64,
        provenance: Result<EntryPreviewProvenance, SourceEvidenceError>,
    ) -> Self {
        let text = decode(&bytes);
        let lines = lines(&text);
        Self {
            bytes,
            size,
            text,
            lines,
            provenance,
        }
    }

    /// Returns source evidence for the retained bytes, their displays and size facts.
    ///
    /// # Errors
    /// Returns an error if the source evidence could not be captured.
    pub const fn provenance(&self) -> Result<&EntryPreviewProvenance, SourceEvidenceError> {
        match &self.provenance {
            Ok(proof) => Ok(proof),
            Err(error) => Err(*error),
        }
    }

    /// Returns the bytes read.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the size of the whole Entry.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Returns whether fewer bytes were read than the Entry holds.
    #[must_use]
    pub fn is_truncated(&self) -> bool {
        u64::try_from(self.bytes.len()).is_ok_and(|read| read < self.size)
    }

    /// Returns the bytes decoded as UTF-8, or as UTF-16 after its byte order mark, with undecodable
    /// bytes and control characters other than line breaks and tabs shown as a middle dot.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the number of lines of the text.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns line `index` of the text without its line break, empty when there is none.
    #[must_use]
    pub fn line(&self, index: usize) -> &str {
        self.lines
            .get(index)
            .and_then(|range| self.text.get(range.clone()))
            .unwrap_or_default()
    }

    /// Returns the number of rows of the hex view.
    #[must_use]
    pub fn hex_row_count(&self) -> usize {
        self.bytes.len().div_ceil(usize::from(HEX_ROW_LEN))
    }

    /// Returns the bytes of row `index` of the hex view.
    #[must_use]
    pub fn hex_row_bytes(&self, index: usize) -> &[u8] {
        self.bytes
            .chunks(usize::from(HEX_ROW_LEN))
            .nth(index)
            .unwrap_or_default()
    }

    /// Returns row `index` of the hex view: the offset, the bytes and their printable characters.
    #[must_use]
    pub fn hex_row(&self, index: usize) -> String {
        let bytes = self.hex_row_bytes(index);
        let row_len = usize::from(HEX_ROW_LEN);
        let offset = index * row_len;
        let mut hex = String::with_capacity(row_len * 3);
        for (position, &byte) in bytes.iter().enumerate() {
            if position > 0 {
                hex.push(' ');
            }
            hex.push(hex_digit(byte >> 4));
            hex.push(hex_digit(byte & 0x0F));
        }
        let ascii: String = bytes.iter().map(|&byte| printable(byte)).collect();
        let width = row_len * 3 - 1;
        format!("{offset:08X}  {hex:<width$}  {ascii}")
    }

    /// Returns what copying `tab` puts on the clipboard: the text, every hex row, or nothing for
    /// the XML, image, DataForge, audio and Record tabs, which do not show these bytes.
    #[must_use]
    pub fn copied(&self, tab: PreviewTab) -> String {
        match tab {
            PreviewTab::Text => self.text.clone(),
            PreviewTab::Hex => self.hex_dump(),
            PreviewTab::Xml
            | PreviewTab::Image
            | PreviewTab::DataForge
            | PreviewTab::Audio
            | PreviewTab::Model
            | PreviewTab::ModelInfo
            | PreviewTab::Record
            | PreviewTab::Incoming => String::new(),
        }
    }

    /// Returns every row of the hex view, each ended by a line break.
    #[must_use]
    pub fn hex_dump(&self) -> String {
        (0..self.hex_row_count())
            .map(|index| self.hex_row(index) + "\n")
            .collect()
    }
}

/// Returns `byte` as an ASCII character when it is printable, otherwise as a dot.
pub(crate) fn printable(byte: u8) -> char {
    if byte.is_ascii_graphic() || byte == b' ' {
        char::from(byte)
    } else {
        '.'
    }
}

fn hex_digit(nibble: u8) -> char {
    char::from_digit(u32::from(nibble), 16).map_or('0', |digit| digit.to_ascii_uppercase())
}

fn decode(bytes: &[u8]) -> String {
    let text = match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [0xFF, 0xFE, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, u16::from_be_bytes),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    };
    text.chars()
        .map(|c| {
            if c == char::REPLACEMENT_CHARACTER
                || c.is_control() && !matches!(c, '\n' | '\r' | '\t')
            {
                PLACEHOLDER
            } else {
                c
            }
        })
        .collect()
}

fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    let (pairs, _odd) = bytes.as_chunks::<2>();
    char::decode_utf16(pairs.iter().map(|&pair| unit(pair)))
        .map(|decoded| decoded.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

fn lines(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    while let Some(offset) = bytes
        .get(start..)
        .and_then(|rest| rest.iter().position(|&byte| matches!(byte, b'\n' | b'\r')))
    {
        let end = start + offset;
        lines.push(start..end);
        start = if bytes.get(end..end + 2) == Some(b"\r\n") {
            end + 2
        } else {
            end + 1
        };
    }
    if start < bytes.len() {
        lines.push(start..bytes.len());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::PreviewContents;

    fn lines(contents: &PreviewContents) -> Vec<&str> {
        (0..contents.line_count())
            .map(|index| contents.line(index))
            .collect()
    }

    fn contents(bytes: Vec<u8>, size: u64) -> PreviewContents {
        let origin = super::EntryPreviewProvenance::capture(None, bytes.len() as u64);
        PreviewContents::new(bytes, size, origin)
    }

    #[test]
    fn lines_end_at_either_kind_of_line_break() {
        let contents = contents(b"one\r\ntwo\n\nthree".to_vec(), 15);

        assert_eq!(lines(&contents), ["one", "two", "", "three"], "lines");
    }

    #[test]
    fn a_lone_carriage_return_ends_a_line_too() {
        let contents = contents(b"one\rtwo\r\r\nthree\r".to_vec(), 16);

        assert_eq!(lines(&contents), ["one", "two", "", "three"], "lines");
    }

    #[test]
    fn a_byte_order_mark_selects_the_encoding_and_is_dropped() {
        let utf8 = contents(b"\xEF\xBB\xBFgr\xC3\xBC\xC3\x9F".to_vec(), 9);
        let utf16 = contents(b"\xFF\xFEh\x00\xE4\x00".to_vec(), 6);
        let big_endian = contents(b"\xFE\xFF\x00h\x00i".to_vec(), 6);

        assert_eq!(utf8.text(), "grüß", "UTF-8");
        assert_eq!(utf16.text(), "hä", "UTF-16 LE");
        assert_eq!(big_endian.text(), "hi", "UTF-16 BE");
    }

    #[test]
    fn control_characters_and_invalid_bytes_are_replaced() {
        let contents = contents(b"a\x00b\tc\xFF".to_vec(), 6);

        assert_eq!(contents.text(), "a\u{B7}b\tc\u{B7}", "replaced");
    }

    #[test]
    fn hex_rows_are_padded_so_the_characters_line_up() {
        let contents = contents(b"0123456789ABCDEFxyz".to_vec(), 19);

        assert_eq!(
            contents.hex_row(1),
            format!("00000010  78 79 7A{}  xyz", " ".repeat(39)),
            "short row"
        );
        assert_eq!(contents.hex_row_count(), 2, "rows");
    }

    #[test]
    fn the_preview_is_truncated_only_when_bytes_were_left_out() {
        assert!(contents(vec![0; 4], 5).is_truncated(), "left out");
        assert!(!contents(vec![0; 4], 4).is_truncated(), "complete");
    }
}
