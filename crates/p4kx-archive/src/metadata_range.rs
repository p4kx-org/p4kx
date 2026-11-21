// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::format_error::FormatError;

/// A checked half-open range relative to its immediately owning Archive source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataRange {
    offset: u64,
    len: u64,
}

impl MetadataRange {
    pub(crate) fn available_prefix(
        offset: u64,
        declared_len: u64,
        source_len: u64,
    ) -> Option<Self> {
        let left = source_len.checked_sub(offset)?;
        Some(Self {
            offset,
            len: left.min(declared_len),
        })
    }

    pub(crate) fn checked(offset: u64, len: u64, source_len: u64) -> Result<Self, FormatError> {
        if offset.checked_add(len).is_none_or(|end| end > source_len) {
            return Err(FormatError::corrupt(
                offset,
                "metadata range exceeds its owning source",
            ));
        }
        Ok(Self { offset, len })
    }

    /// Returns the owning-source offset, without a backing-file coordinate conversion.
    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }

    /// Returns the checked range length.
    #[must_use]
    pub const fn len(self) -> u64 {
        self.len
    }

    /// Returns whether the declared range contains no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    pub(crate) const fn end(self) -> u64 {
        self.offset + self.len
    }
}
