// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use p4kx_extraction::{
    Archive, Entry, EntryReadError, MIN_TEXT_LEN, TextEncoding, find_entry, read_bytes,
    read_text_observed,
};

use crate::content_origin::BytesFieldOrigins;
use crate::content_text_origin::TextFieldOrigins;
use crate::error::ToolError;
use crate::shapes::{BytesResult, TextResult};

/// The text length a call gives none.
pub(crate) const DEFAULT_TEXT_LEN: usize = 32 << 10;
/// The largest text length; a larger `max_bytes` is lowered to it.
pub(crate) const MAX_TEXT_LEN: usize = 256 << 10;
/// The byte length a call gives none.
pub(crate) const DEFAULT_BYTES_LEN: usize = 16 << 10;
/// The largest byte length, 256 KiB once base64 encoded; a larger `max_bytes` is lowered to it.
pub(crate) const MAX_BYTES_LEN: usize = 192 << 10;

/// A requested length clamped to its allowed range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Length {
    value: usize,
    clamped: bool,
}

impl Length {
    /// Returns `requested`, or `default` when absent, within `min..=max`.
    pub(crate) fn new(requested: Option<u32>, default: usize, min: usize, max: usize) -> Self {
        let Some(requested) = requested else {
            return Self {
                value: default,
                clamped: false,
            };
        };
        let wanted = usize::try_from(requested).unwrap_or(usize::MAX);
        let value = wanted.clamp(min, max);
        Self {
            value,
            clamped: value != wanted,
        }
    }
}

/// Returns a range of the text of the Entry at `path`; `whole` runs before an Entry is decoded as
/// a whole and its result is held while that lasts.
pub(crate) fn text<G>(
    archive: &Archive,
    path: &str,
    offset: u64,
    requested: Option<u32>,
    whole: impl FnOnce() -> G,
) -> Result<TextResult, ToolError> {
    let length = Length::new(requested, DEFAULT_TEXT_LEN, MIN_TEXT_LEN, MAX_TEXT_LEN);
    let entry = entry(archive, path)?;
    let observed = read_text_observed(archive, entry, offset, length.value, whole)
        .map_err(|error| failed(entry, error))?;
    let (field_origins, provenance) =
        TextFieldOrigins::capture(archive, entry, &observed).map_err(ToolError::EntryEvidence)?;
    let range = observed.page;
    let next_offset = range.next_offset();
    Ok(TextResult {
        field_origins,
        provenance,
        archive: archive.path().display().to_string(),
        path: entry.path().to_owned(),
        encoding: encoding_name(range.encoding),
        size: range.size,
        offset: range.offset,
        length: range.text.len(),
        text: range.text,
        more: next_offset.is_some(),
        next_offset,
        clamped: length.clamped,
        max_bytes: length.value,
    })
}

/// Returns a range of the data of the Entry at `path`, base64 encoded.
pub(crate) fn bytes(
    archive: &Archive,
    path: &str,
    offset: u64,
    requested: Option<u32>,
) -> Result<BytesResult, ToolError> {
    let length = Length::new(requested, DEFAULT_BYTES_LEN, 1, MAX_BYTES_LEN);
    let entry = entry(archive, path)?;
    let range =
        read_bytes(archive, entry, offset, length.value).map_err(|error| failed(entry, error))?;
    let next_offset = range.next_offset();
    let (field_origins, provenance) =
        BytesFieldOrigins::capture(archive, entry, &range).map_err(ToolError::EntryEvidence)?;
    Ok(BytesResult {
        archive: archive.path().display().to_string(),
        path: entry.path().to_owned(),
        size: range.size,
        offset: range.offset,
        length: range.bytes.len(),
        encoding: "base64",
        data: STANDARD.encode(&range.bytes),
        more: next_offset.is_some(),
        next_offset,
        clamped: length.clamped,
        max_bytes: length.value,
        field_origins,
        provenance,
    })
}

fn entry<'a>(archive: &'a Archive, path: &str) -> Result<&'a Entry, ToolError> {
    find_entry(archive, path).ok_or_else(|| ToolError::UnknownEntry(path.to_owned()))
}

fn failed(entry: &Entry, error: EntryReadError) -> ToolError {
    match error {
        EntryReadError::Binary => ToolError::Binary(entry.path().to_owned()),
        EntryReadError::TooLarge { limit } => ToolError::TooLarge {
            path: entry.path().to_owned(),
            limit,
        },
        source => ToolError::Read {
            path: entry.path().to_owned(),
            source,
        },
    }
}

const fn encoding_name(encoding: TextEncoding) -> &'static str {
    match encoding {
        TextEncoding::Utf8 => "utf-8",
        TextEncoding::Utf16Le => "utf-16le",
        TextEncoding::Utf16Be => "utf-16be",
        TextEncoding::CryXml => "cryxml",
    }
}

#[cfg(test)]
mod tests {
    use super::Length;

    #[test]
    fn lengths_outside_the_range_are_clamped_and_say_so() {
        assert_eq!(
            Length::new(None, 8, 1, 16),
            Length {
                value: 8,
                clamped: false
            },
            "the default"
        );
        assert_eq!(
            Length::new(Some(0), 8, 1, 16),
            Length {
                value: 1,
                clamped: true
            },
            "raised"
        );
        assert_eq!(
            Length::new(Some(u32::MAX), 8, 1, 16),
            Length {
                value: 16,
                clamped: true
            },
            "lowered"
        );
        assert_eq!(
            Length::new(Some(16), 8, 1, 16),
            Length {
                value: 16,
                clamped: false
            },
            "the cap itself"
        );
    }
}
