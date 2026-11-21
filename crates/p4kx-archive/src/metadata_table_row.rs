// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::metadata_region::{MetadataRegion, MetadataRegionStatus};
use crate::metadata_span::MetadataSpan;

/// Validation of a declared progress value against an available physical Entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetadataProgressBound {
    /// The corresponding source Entry was not admitted into this view.
    Unavailable,
    /// Declared progress does not exceed the stored Entry size.
    WithinStoredSize,
    /// Declared progress exceeds the stored Entry size.
    ExceedsStoredSize,
}

/// Meaning of the optional reserved word in a progress record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetadataReservedWord {
    /// The record format has no reserved word.
    NotPresent,
    /// The original word is zero.
    Zero,
    /// The original word is nonzero and remains retained as malformed metadata.
    Nonzero,
}

/// Raw installation progress with checks separate from source-word presence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataInstallValue {
    pub(crate) written: u64,
    pub(crate) reserved: Option<u32>,
    pub(crate) bound: MetadataProgressBound,
}

impl MetadataInstallValue {
    /// Returns the original declared progress word, not verified payload bytes.
    #[must_use]
    pub const fn written(self) -> u64 {
        self.written
    }
    /// Returns the original reserved word, absent only in the v2 eight-byte format.
    #[must_use]
    pub const fn reserved(self) -> Option<u32> {
        self.reserved
    }
    /// Returns progress validation without fabricating a bound for omitted Entries.
    #[must_use]
    pub const fn bound(self) -> MetadataProgressBound {
        self.bound
    }
    /// Returns whether the reserved word is absent, zero or malformed nonzero data.
    #[must_use]
    pub const fn reserved_status(self) -> MetadataReservedWord {
        match self.reserved {
            None => MetadataReservedWord::NotPresent,
            Some(0) => MetadataReservedWord::Zero,
            Some(_) => MetadataReservedWord::Nonzero,
        }
    }
}

/// Validation of one declared free extent, without a filesystem allocation claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetadataFreeStatus {
    /// A nonempty declared extent fits the owning source.
    WithinSource,
    /// The source record declares a zero-size extent.
    ZeroSize,
    /// The extent does not fit the owning source.
    OutsideSource,
    /// The extent's offset and size overflow.
    Overflow,
}

/// Original free-range words and their independently checked extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataFreeValue {
    pub(crate) extent: MetadataRegion,
}

impl MetadataFreeValue {
    /// Returns exact source words and available range facts without granting payload access.
    #[must_use]
    pub const fn extent(self) -> MetadataRegion {
        self.extent
    }
    /// Returns structural validation of the declared free range.
    #[must_use]
    pub const fn status(self) -> MetadataFreeStatus {
        match self.extent.status() {
            MetadataRegionStatus::Complete if self.extent.declared_len() == 0 => {
                MetadataFreeStatus::ZeroSize
            }
            MetadataRegionStatus::Complete => MetadataFreeStatus::WithinSource,
            MetadataRegionStatus::Overflow => MetadataFreeStatus::Overflow,
            MetadataRegionStatus::Truncated | MetadataRegionStatus::OutsideSource => {
                MetadataFreeStatus::OutsideSource
            }
        }
    }
}

/// Typed values of one original installation or free-list record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetadataTableValue {
    /// Declared installation progress and an optional reserved word.
    Installation(MetadataInstallValue),
    /// Declared free-space offset and length.
    FreeRange(MetadataFreeValue),
}

/// A source-owned table row retaining its original bytes and physical ordinal.
#[derive(Debug, Clone, Copy)]
pub struct MetadataTableRow<'source> {
    pub(crate) ordinal: u64,
    pub(crate) raw: MetadataSpan<'source>,
    pub(crate) value: MetadataTableValue,
}

impl<'source> MetadataTableRow<'source> {
    /// Returns the original table ordinal, without reindexing omitted folders.
    #[must_use]
    pub const fn ordinal(self) -> u64 {
        self.ordinal
    }
    /// Returns the original fixed record bytes.
    #[must_use]
    pub const fn raw(self) -> MetadataSpan<'source> {
        self.raw
    }
    /// Returns the typed raw values and separate validation facts.
    #[must_use]
    pub const fn value(self) -> MetadataTableValue {
        self.value
    }
}
