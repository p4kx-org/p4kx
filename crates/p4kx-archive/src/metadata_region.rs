// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::metadata_range::MetadataRange;

/// Whether a declared metadata extent fits in its owning source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetadataRegionStatus {
    /// All declared bytes are available.
    Complete,
    /// Only a prefix of the declared bytes lies in the source.
    Truncated,
    /// The declared start lies outside the source.
    OutsideSource,
    /// The declared offset and length overflow their source integer width.
    Overflow,
}

/// Exact extent declarations with separately checked available bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataRegion {
    offset: u64,
    declared_len: u64,
    available: Option<MetadataRange>,
    status: MetadataRegionStatus,
}

impl MetadataRegion {
    pub(crate) fn of(offset: u64, declared_len: u64, source_len: u64) -> Self {
        let left = source_len.checked_sub(offset);
        let available = MetadataRange::available_prefix(offset, declared_len, source_len);
        let status = match (offset.checked_add(declared_len), left) {
            (None, _) => MetadataRegionStatus::Overflow,
            (_, None) => MetadataRegionStatus::OutsideSource,
            (Some(end), Some(_)) if end > source_len => MetadataRegionStatus::Truncated,
            _ => MetadataRegionStatus::Complete,
        };
        Self {
            offset,
            declared_len,
            available,
            status,
        }
    }

    /// Returns the unmodified source offset declaration.
    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }

    /// Returns the complete source length declaration, including malformed values.
    #[must_use]
    pub const fn declared_len(self) -> u64 {
        self.declared_len
    }

    /// Returns the checked available prefix, independently of declaration validity.
    #[must_use]
    pub const fn available(self) -> Option<MetadataRange> {
        self.available
    }

    /// Returns whether the complete declared extent is available.
    #[must_use]
    pub const fn status(self) -> MetadataRegionStatus {
        self.status
    }
}
