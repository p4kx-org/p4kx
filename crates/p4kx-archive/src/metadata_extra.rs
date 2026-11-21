// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::metadata_span::MetadataSpan;

/// Coverage of an extra field under the reader's total-length-includes-header convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetadataExtraStatus {
    /// Header and complete declared body fit in the original extra section.
    Complete,
    /// The extra section ends inside its four-byte header.
    ShortHeader,
    /// The declared total length is less than the four-byte header.
    LengthBeforeHeader,
    /// Only a prefix of the declared body is available.
    TruncatedBody,
}

/// One ordered raw extra field, including duplicate and unknown IDs.
#[derive(Debug, Clone, Copy)]
pub struct MetadataExtra<'source> {
    pub(crate) ordinal: u64,
    pub(crate) id: Option<u16>,
    pub(crate) declared_total_len: Option<u16>,
    pub(crate) raw: MetadataSpan<'source>,
    pub(crate) body: Option<MetadataSpan<'source>>,
    pub(crate) status: MetadataExtraStatus,
}

impl<'source> MetadataExtra<'source> {
    /// Returns the observed physical extra-field ordinal without deduplicating IDs.
    #[must_use]
    pub const fn ordinal(self) -> u64 {
        self.ordinal
    }
    /// Returns the source ID when its complete word is available.
    #[must_use]
    pub const fn id(self) -> Option<u16> {
        self.id
    }
    /// Returns the original total-length word, including its four-byte header.
    #[must_use]
    pub const fn declared_total_len(self) -> Option<u16> {
        self.declared_total_len
    }
    /// Returns all available raw bytes of this field or malformed remaining section.
    #[must_use]
    pub const fn raw(self) -> MetadataSpan<'source> {
        self.raw
    }
    /// Returns the body or its explicitly partial available prefix.
    #[must_use]
    pub const fn body(self) -> Option<MetadataSpan<'source>> {
        self.body
    }
    /// Returns framing coverage independently of semantic interpretation of the ID.
    #[must_use]
    pub const fn status(self) -> MetadataExtraStatus {
        self.status
    }
}
