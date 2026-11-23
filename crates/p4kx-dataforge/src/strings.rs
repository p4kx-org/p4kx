// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::ffi::CStr;

use crate::{DataForgeError, ValueError};

/// A table of NUL-terminated UTF-8 strings addressed by byte offset.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StringTable<'a> {
    bytes: &'a [u8],
    name: &'static str,
}

impl<'a> StringTable<'a> {
    pub(crate) const fn new(bytes: &'a [u8], name: &'static str) -> Self {
        Self { bytes, name }
    }

    pub(crate) const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Returns the string starting at `offset`.
    pub(crate) fn get(&self, offset: u32) -> Result<&'a str, DataForgeError> {
        self.read(offset).0.ok_or(DataForgeError::InvalidString {
            table: self.name,
            offset,
        })
    }

    /// Returns the string starting at `offset` as a field value.
    pub(crate) fn value(&self, offset: u32) -> Result<&'a str, ValueError> {
        self.value_with_extent(offset).0
    }

    /// Returns the decoded value and the original inspected range, including its terminator.
    pub(crate) fn value_with_extent(
        &self,
        offset: u32,
    ) -> (Result<&'a str, ValueError>, Option<&'a [u8]>) {
        let (text, examined) = self.read(offset);
        (
            text.ok_or(ValueError::InvalidString {
                table: self.name,
                offset,
            }),
            examined,
        )
    }

    fn read(&self, offset: u32) -> (Option<&'a str>, Option<&'a [u8]>) {
        let tail = usize::try_from(offset)
            .ok()
            .and_then(|start| self.bytes.get(start..));
        let Some(tail) = tail else {
            return (None, None);
        };
        let Ok(text) = CStr::from_bytes_until_nul(tail) else {
            return (None, Some(tail));
        };
        (text.to_str().ok(), Some(text.to_bytes_with_nul()))
    }
}

const XML_EXTENSION: &str = ".xml";

/// Returns `path` without a trailing `.xml` in any case.
pub(crate) fn strip_xml(path: &str) -> &str {
    path.len()
        .checked_sub(XML_EXTENSION.len())
        .and_then(|cut| path.split_at_checked(cut))
        .filter(|(_, extension)| extension.eq_ignore_ascii_case(XML_EXTENSION))
        .map_or(path, |(stem, _)| stem)
}

/// Orders texts ignoring ASCII case.
pub(crate) fn compare_ignoring_case(left: &str, right: &str) -> Ordering {
    let key = |byte: u8| byte.to_ascii_lowercase();
    left.bytes().map(key).cmp(right.bytes().map(key))
}

/// Orders record paths ignoring ASCII case and treating `\` as `/`.
pub(crate) fn compare_paths(left: &str, right: &str) -> Ordering {
    let key = |byte: u8| {
        if byte == b'\\' {
            b'/'
        } else {
            byte.to_ascii_lowercase()
        }
    };
    left.bytes().map(key).cmp(right.bytes().map(key))
}
