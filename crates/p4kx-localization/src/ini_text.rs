// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::str;

use crate::{GlobalIni, LocalizationError, LocalizationLimits};

#[path = "ini_text_decoder.rs"]
mod decoder;

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";
const UTF16_LE_BOM: &[u8] = b"\xff\xfe";
const UTF16_BE_BOM: &[u8] = b"\xfe\xff";

/// The encoding a `global.ini` was stored in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IniEncoding {
    /// UTF-8 without a byte order mark.
    Utf8,
    /// UTF-8 with a byte order mark, as the game stores its files.
    Utf8Bom,
    /// UTF-16, little endian.
    Utf16Le,
    /// UTF-16, big endian.
    Utf16Be,
}

impl IniEncoding {
    const fn is_utf8(self) -> bool {
        matches!(self, Self::Utf8 | Self::Utf8Bom)
    }
}

/// The text of a `global.ini` decoded to UTF-8, borrowed from the bytes where they already are.
///
/// UTF-8 with or without a byte order mark and UTF-16 of either byte order, with a byte order
/// mark or, when the first byte is non-zero and the second zero (or the reverse), without, are
/// read. Invalid sequences become U+FFFD. A trailing partial UTF-16 code unit marks the text
/// as lossy.
#[derive(Debug, Clone)]
pub struct IniText<'a> {
    text: Cow<'a, str>,
    encoding: IniEncoding,
    lossy: bool,
}

impl<'a> IniText<'a> {
    /// Decodes `bytes`, borrowing them when they are valid UTF-8.
    ///
    /// # Errors
    ///
    /// Returns [`LocalizationError::DecodeOutOfMemory`] when decoded text cannot be reserved.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, LocalizationError> {
        Self::decode_with_limits(bytes, LocalizationLimits::UNLIMITED)
    }

    /// Decodes `bytes` with the caller's source and decoded byte budgets.
    ///
    /// # Errors
    ///
    /// Returns a budget error when a configured byte budget is exceeded or
    /// [`LocalizationError::DecodeOutOfMemory`] when decoded text cannot be reserved.
    pub fn decode_with_limits(
        bytes: &'a [u8],
        limits: LocalizationLimits,
    ) -> Result<Self, LocalizationError> {
        limits.check_source_bytes(bytes.len())?;
        let (encoding, body) = detect(bytes);
        if encoding.is_utf8()
            && let Ok(text) = str::from_utf8(body)
        {
            limits.check_decoded_bytes(text.len())?;
            return Ok(Self {
                text: Cow::Borrowed(text),
                encoding,
                lossy: false,
            });
        }
        decoder::decode(body, encoding, limits)
    }

    /// Decodes the owned `bytes`, reusing their allocation when they are valid UTF-8.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`IniText::decode`].
    pub fn decode_vec(bytes: Vec<u8>) -> Result<IniText<'static>, LocalizationError> {
        Self::decode_vec_with_limits(bytes, LocalizationLimits::UNLIMITED)
    }

    /// Decodes owned `bytes` with the caller's source and decoded byte budgets.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`IniText::decode_with_limits`].
    pub fn decode_vec_with_limits(
        bytes: Vec<u8>,
        limits: LocalizationLimits,
    ) -> Result<IniText<'static>, LocalizationError> {
        limits.check_source_bytes(bytes.len())?;
        let (encoding, body) = detect(&bytes);
        if encoding.is_utf8() {
            let prefix = bytes.len() - body.len();
            return match String::from_utf8(bytes) {
                Ok(mut text) => {
                    limits.check_decoded_bytes(text.len() - prefix)?;
                    text.replace_range(..prefix, "");
                    Ok(IniText {
                        text: Cow::Owned(text),
                        encoding,
                        lossy: false,
                    })
                }
                Err(error) => {
                    let (encoding, body) = detect(error.as_bytes());
                    decoder::decode(body, encoding, limits)
                }
            };
        }
        decoder::decode(body, encoding, limits)
    }

    /// Returns the decoded text, without the byte order mark.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the encoding the bytes were stored in.
    #[must_use]
    pub const fn encoding(&self) -> IniEncoding {
        self.encoding
    }

    /// Returns whether bytes that are no valid text were replaced by U+FFFD.
    #[must_use]
    pub const fn is_lossy(&self) -> bool {
        self.lossy
    }

    /// Indexes the texts by localisation key.
    ///
    /// # Errors
    ///
    /// Returns [`LocalizationError::OutOfMemory`] when index storage cannot be reserved.
    pub fn index(&self) -> Result<GlobalIni<'_>, LocalizationError> {
        GlobalIni::parse(&self.text)
    }

    /// Indexes the texts with the caller's decoded byte and distinct key budgets.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`GlobalIni::parse_with_limits`].
    pub fn index_with_limits(
        &self,
        limits: LocalizationLimits,
    ) -> Result<GlobalIni<'_>, LocalizationError> {
        GlobalIni::parse_with_limits(&self.text, limits)
    }
}

fn detect(bytes: &[u8]) -> (IniEncoding, &[u8]) {
    let boms = [
        (UTF8_BOM, IniEncoding::Utf8Bom),
        (UTF16_LE_BOM, IniEncoding::Utf16Le),
        (UTF16_BE_BOM, IniEncoding::Utf16Be),
    ];
    for (bom, encoding) in boms {
        if let Some(body) = bytes.strip_prefix(bom) {
            return (encoding, body);
        }
    }
    match bytes {
        [first, 0, ..] if *first != 0 => (IniEncoding::Utf16Le, bytes),
        [0, second, ..] if *second != 0 => (IniEncoding::Utf16Be, bytes),
        _ => (IniEncoding::Utf8, bytes),
    }
}
