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
        usize::try_from(offset)
            .ok()
            .and_then(|start| self.bytes.get(start..))
            .and_then(|tail| CStr::from_bytes_until_nul(tail).ok())
            .and_then(|text| text.to_str().ok())
            .ok_or(DataForgeError::InvalidString {
                table: self.name,
                offset,
            })
    }

    /// Returns the string starting at `offset` as a field value.
    pub(crate) fn value(&self, offset: u32) -> Result<&'a str, ValueError> {
        self.get(offset).map_err(|_| ValueError::InvalidString {
            table: self.name,
            offset,
        })
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
