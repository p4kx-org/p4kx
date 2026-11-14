// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Write};
use std::ops::Range;

use p4kx_archive::{Archive, Entry};
use p4kx_cryxml::XmlWriteField;

use crate::convert::is_cryxml;
use crate::entry_content::{EntryReadError, read_up_to};
use crate::text_window::{Window, convert_into_window};

/// The fewest bytes of text a read returns while text remains, enough for any one character.
pub const MIN_TEXT_LEN: usize = 4;
const MAX_WHOLE_TEXT_LEN: u64 = 32 << 20;
const MAX_CONVERTED_XML_LEN: u64 = 256 << 20;
const SNIFF_LEN: u64 = 8 << 10;
const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const MAX_CONTINUATION_LEN: usize = 3;

/// How the text of an Entry is decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    /// UTF-8, with or without a byte order mark.
    Utf8,
    /// UTF-16 little endian after its byte order mark.
    Utf16Le,
    /// UTF-16 big endian after its byte order mark.
    Utf16Be,
    /// `CryXml`, converted to UTF-8 XML.
    CryXml,
}

/// A range of the text of an Entry, measured in bytes of its UTF-8 form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryText {
    /// How the Entry was decoded.
    pub encoding: TextEncoding,
    /// The offset of the first byte of `text` in the whole text.
    pub offset: u64,
    /// The text read.
    pub text: String,
    /// The size of the whole text in UTF-8 bytes, without a byte order mark.
    pub size: u64,
}

/// One returned text page with its original decoded Entry evidence.
#[derive(Debug)]
#[non_exhaustive]
pub struct ObservedEntryText {
    /// The returned UTF-8 text page.
    pub page: EntryText,
    /// Input used to recognize or decode the page.
    pub evidence: TextReadEvidence,
}

/// Original decoded Entry spans examined by the text reader.
#[derive(Debug)]
#[non_exhaustive]
pub enum TextReadEvidence {
    /// The source Entry held UTF-8 bytes.
    #[non_exhaustive]
    Utf8 {
        /// Prefix inspected to recognize the encoding and text format.
        prefix: Range<u64>,
        /// Bytes compared against the optional byte order mark.
        bom_probe: Range<u64>,
        /// Bytes inspected to align the requested starting offset.
        start_probe: Range<u64>,
        /// Range inspected for the returned page and its character boundaries.
        inspected: Range<u64>,
        /// Range retained verbatim in the returned text.
        payload: Range<u64>,
    },
    /// The reader converted the entire UTF-16 or CryXml input to UTF-8.
    #[non_exhaustive]
    Transcoded {
        /// Range passed to the converter in decoded Entry bytes.
        input: Range<u64>,
        /// Original CryXml fields intersecting the retained UTF-8 page.
        xml_fields: Vec<XmlWriteField>,
    },
}
impl EntryText {
    /// Returns the offset of the next range, or `None` when the range reaches the end.
    #[must_use]
    pub fn next_offset(&self) -> Option<u64> {
        let end = self.offset.saturating_add(self.text.len() as u64);
        (end < self.size).then_some(end)
    }
}

/// An encoding that is decoded from the whole Entry.
#[derive(Debug, Clone, Copy)]
enum Whole {
    Utf16Le,
    Utf16Be,
    CryXml,
}

impl Whole {
    fn of(head: &[u8]) -> Option<Self> {
        match head {
            [0xFF, 0xFE, ..] => Some(Self::Utf16Le),
            [0xFE, 0xFF, ..] => Some(Self::Utf16Be),
            start if is_cryxml(start) => Some(Self::CryXml),
            _ => None,
        }
    }

    const fn encoding(self) -> TextEncoding {
        match self {
            Self::Utf16Le => TextEncoding::Utf16Le,
            Self::Utf16Be => TextEncoding::Utf16Be,
            Self::CryXml => TextEncoding::CryXml,
        }
    }
}

/// Reads the text of `entry` from byte `offset` of its UTF-8 form, at most `max_len` bytes.
///
/// A `max_len` below [`MIN_TEXT_LEN`] counts as that. The range starts and ends on whole
/// characters: an `offset` inside a character moves to the next one. `CryXml` is converted to
/// XML and UTF-16 is recognised by its byte order mark; both are decoded as a whole, and `whole`
/// is called before that and its result held until the range is cut, so a caller can bound how
/// many run at once. Anything else has to be UTF-8 and is read only around the range. NUL,
/// control characters other than tab, line feed, form feed and carriage return, DEL and invalid
/// UTF-8 make data binary, so Latin-1 text with bytes above 127 reads as binary.
///
/// # Errors
///
/// Returns [`EntryReadError::Binary`] for data that is no text, [`EntryReadError::TooLarge`] when
/// `CryXml` or UTF-16 is above 32 MiB or decodes to more than 256 MiB,
/// [`EntryReadError::OffsetBeyondEnd`] when `offset` exceeds the text, and another
/// [`EntryReadError`] when the data cannot be read.
pub fn read_text<G>(
    archive: &Archive,
    entry: &Entry,
    offset: u64,
    max_len: usize,
    whole: impl FnOnce() -> G,
) -> Result<EntryText, EntryReadError> {
    read_text_page(archive, entry, offset, max_len, whole, XmlSites::Discard)
        .map(|observed| observed.page)
}

/// Reads a text page together with the decoded Entry spans used by its decoder.
///
/// # Errors
/// Returns the same errors as [`read_text`].
pub fn read_text_observed<G>(
    archive: &Archive,
    entry: &Entry,
    offset: u64,
    max_len: usize,
    whole: impl FnOnce() -> G,
) -> Result<ObservedEntryText, EntryReadError> {
    read_text_page(archive, entry, offset, max_len, whole, XmlSites::Retain)
}

#[derive(Clone, Copy)]
enum XmlSites {
    Discard,
    Retain,
}

fn read_text_page<G>(
    archive: &Archive,
    entry: &Entry,
    offset: u64,
    max_len: usize,
    whole: impl FnOnce() -> G,
    sites: XmlSites,
) -> Result<ObservedEntryText, EntryReadError> {
    let max_len = max_len.max(MIN_TEXT_LEN);
    let size = entry.uncompressed_size();
    let mut reader = archive.open_entry(entry)?;
    let head = read_up_to(&mut reader, size.min(SNIFF_LEN))?;
    let Some(kind) = Whole::of(&head) else {
        return utf8_range(archive, entry, (head, reader), offset, max_len);
    };
    if size > MAX_WHOLE_TEXT_LEN {
        return Err(EntryReadError::TooLarge {
            limit: MAX_WHOLE_TEXT_LEN,
        });
    }
    let permit = whole();
    let mut data = head;
    let rest = size.saturating_sub(data.len() as u64);
    data.reserve(usize::try_from(rest).unwrap_or(0));
    reader
        .take(rest)
        .read_to_end(&mut data)
        .map_err(EntryReadError::Read)?;
    let keep = max_len.saturating_add(MAX_CONTINUATION_LEN);
    let mut xml_fields = Vec::new();
    let window = match kind {
        Whole::Utf16Le => utf16(&data, u16::from_le_bytes, offset, keep)?,
        Whole::Utf16Be => utf16(&data, u16::from_be_bytes, offset, keep)?,
        Whole::CryXml => cryxml_window(&data, offset, keep, sites, &mut xml_fields)?,
    };
    let transcoded_bytes = data.len() as u64;
    drop(data);
    drop(permit);
    let text_size = window.written();
    if offset > text_size {
        return Err(EntryReadError::OffsetBeyondEnd {
            offset,
            size: text_size,
        });
    }
    let page = cut(
        kind.encoding(),
        window.into_kept(),
        offset,
        text_size,
        max_len,
    )?;
    let retained = page.offset..page.offset + page.text.len() as u64;
    xml_fields.retain(|field| overlaps(field.range(), &retained));
    Ok(ObservedEntryText {
        page,
        evidence: TextReadEvidence::Transcoded {
            input: 0..transcoded_bytes,
            xml_fields,
        },
    })
}

fn cryxml_window(
    data: &[u8],
    offset: u64,
    keep: usize,
    sites: XmlSites,
    fields: &mut Vec<XmlWriteField>,
) -> Result<Window, EntryReadError> {
    let buffered = offset..offset.saturating_add(keep as u64);
    convert_into_window(data, offset, keep, MAX_CONVERTED_XML_LEN, |field| {
        if matches!(sites, XmlSites::Retain) && overlaps(field.range(), &buffered) {
            fields.try_reserve(1).map_err(io::Error::other)?;
            fields.push(field);
        }
        Ok(())
    })
}

fn overlaps(first: &Range<u64>, second: &Range<u64>) -> bool {
    first.start.max(second.start) < first.end.min(second.end)
}

fn utf8_range(
    archive: &Archive,
    entry: &Entry,
    (head, reader): (Vec<u8>, impl Read),
    offset: u64,
    max_len: usize,
) -> Result<ObservedEntryText, EntryReadError> {
    let size = entry.uncompressed_size();
    let prefix = 0..head.len() as u64;
    if !is_text(&head, prefix.end == size) {
        return Err(EntryReadError::Binary);
    }
    let bom_probe = 0..if head.len() >= UTF8_BOM.len() {
        UTF8_BOM.len() as u64
    } else {
        0
    };
    let bom = if head.starts_with(UTF8_BOM) {
        UTF8_BOM.len() as u64
    } else {
        0
    };
    let text_size = size - bom;
    let remaining = text_size
        .checked_sub(offset)
        .ok_or(EntryReadError::OffsetBeyondEnd {
            offset,
            size: text_size,
        })?;
    let window_len = remaining.min((max_len as u64).saturating_add(MAX_CONTINUATION_LEN as u64));
    let start = bom + offset;
    let window = match usize::try_from(start)
        .ok()
        .and_then(|start| head.get(start..))
    {
        Some(buffered) => read_up_to(&mut buffered.chain(reader), window_len)?,
        None => read_up_to(&mut archive.open_entry_at(entry, start)?, window_len)?,
    };
    let available = window.len() as u64;
    let page = cut(TextEncoding::Utf8, window, offset, text_size, max_len)?;
    let lead = page.offset - offset;
    let inspected_len = available.min(lead.saturating_add(max_len as u64));
    let start_probe_len = available.min((lead + 1).min(MAX_CONTINUATION_LEN as u64));
    let inspected = start..start + inspected_len;
    let payload_start = bom + page.offset;
    let payload = payload_start..payload_start + page.text.len() as u64;
    Ok(ObservedEntryText {
        page,
        evidence: TextReadEvidence::Utf8 {
            prefix,
            bom_probe,
            start_probe: start..start + start_probe_len,
            inspected,
            payload,
        },
    })
}

/// Cuts the text range out of `window`, the bytes from `offset` of a text of `size` bytes.
fn cut(
    encoding: TextEncoding,
    mut window: Vec<u8>,
    offset: u64,
    size: u64,
    max_len: usize,
) -> Result<EntryText, EntryReadError> {
    let lead = window
        .iter()
        .take(MAX_CONTINUATION_LEN)
        .take_while(|&&byte| is_continuation(byte))
        .count();
    window.truncate(lead.saturating_add(max_len));
    window.drain(..lead);
    let start = offset + lead as u64;
    let at_end = start + window.len() as u64 == size;
    match std::str::from_utf8(&window) {
        Ok(_) => {}
        Err(error) if error.error_len().is_none() && !at_end => {
            window.truncate(error.valid_up_to());
        }
        Err(_) => return Err(EntryReadError::Binary),
    }
    if encoding != TextEncoding::CryXml && window.iter().any(|&byte| is_binary(byte)) {
        return Err(EntryReadError::Binary);
    }
    let text = String::from_utf8(window).map_err(|_| EntryReadError::Binary)?;
    Ok(EntryText {
        encoding,
        offset: start,
        text,
        size,
    })
}

/// Decodes UTF-16 after its byte order mark into a window of its UTF-8 form.
fn utf16(
    data: &[u8],
    unit: fn([u8; 2]) -> u16,
    offset: u64,
    keep: usize,
) -> Result<Window, EntryReadError> {
    let (pairs, odd) = data.get(2..).unwrap_or_default().as_chunks::<2>();
    if !odd.is_empty() {
        return Err(EntryReadError::Binary);
    }
    let mut window = Window::new(offset, keep, MAX_CONVERTED_XML_LEN);
    let mut encoded = [0; 4];
    for decoded in char::decode_utf16(pairs.iter().map(|&pair| unit(pair))) {
        let character = decoded.map_err(|_| EntryReadError::Binary)?;
        if u8::try_from(character).is_ok_and(is_binary) {
            return Err(EntryReadError::Binary);
        }
        window
            .write_all(character.encode_utf8(&mut encoded).as_bytes())
            .map_err(|_| EntryReadError::TooLarge {
                limit: MAX_CONVERTED_XML_LEN,
            })?;
    }
    Ok(window)
}

/// Returns whether `head`, the start of an Entry or all of it when `complete`, is UTF-8 text.
fn is_text(head: &[u8], complete: bool) -> bool {
    if head.iter().any(|&byte| is_binary(byte)) {
        return false;
    }
    match std::str::from_utf8(head) {
        Ok(_) => true,
        Err(error) => error.error_len().is_none() && !complete,
    }
}

/// Returns whether `byte` is a control character that text does not hold.
const fn is_binary(byte: u8) -> bool {
    (byte < 0x20 && !matches!(byte, b'\t' | b'\n' | 0x0C | b'\r')) || byte == 0x7F
}

const fn is_continuation(byte: u8) -> bool {
    byte & 0xC0 == 0x80
}
