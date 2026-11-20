// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::format_error::FormatError;
use crate::local_header_match::{LOCAL_EXTRA_AT, LOCAL_NAME_AT, LocalStart};
use crate::{Entry, MetadataRange};

const ZIP_COMPRESSED_AT: u64 = 20;
const ZIP_EXTRA_LENGTH_AT: u64 = 30;
const ZIP_LOCAL_OFFSET_AT: u64 = 42;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Zip64BoundOffsets {
    pub(crate) compressed_size: Option<u64>,
    pub(crate) local_header_offset: Option<u64>,
    pub(crate) header: Option<u64>,
}

pub(crate) struct ZipBoundWords {
    pub(crate) offsets: Zip64BoundOffsets,
    pub(crate) extra_len: u64,
    pub(crate) has_size_marker: bool,
    pub(crate) has_offset_marker: bool,
}

/// One word actually used in checking a recovered data extent.
#[derive(Debug, Clone, Copy)]
pub struct ObservedRecoveryWord {
    value: u64,
    range: MetadataRange,
}

impl ObservedRecoveryWord {
    /// Returns the stored little-endian value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.value
    }

    /// Returns the value's exact byte range in the owning source.
    #[must_use]
    pub const fn range(self) -> MetadataRange {
        self.range
    }
}

/// Central directory bytes consulted to resolve a ZIP64 extent operand.
#[derive(Debug, Clone, Copy)]
pub struct ObservedRecoveryZip64Selection {
    size_words: MetadataRange,
    local_offset_word: MetadataRange,
    extra_length_word: MetadataRange,
    header: Option<MetadataRange>,
    complete: bool,
}

impl ObservedRecoveryZip64Selection {
    /// Returns the fixed compressed and uncompressed size words controlling ZIP64 word order.
    #[must_use]
    pub const fn size_words(self) -> MetadataRange {
        self.size_words
    }

    /// Returns the fixed local-header offset word.
    #[must_use]
    pub const fn local_offset_word(self) -> MetadataRange {
        self.local_offset_word
    }

    /// Returns the fixed central extra field length word.
    #[must_use]
    pub const fn extra_length_word(self) -> MetadataRange {
        self.extra_length_word
    }

    /// Returns the type and length words of the ZIP64 block that supplied the extent words.
    #[must_use]
    pub const fn header(self) -> Option<MetadataRange> {
        self.header
    }

    /// Returns whether the selection has a winning header or an empty extra field.
    #[must_use]
    pub const fn complete(self) -> bool {
        self.complete
    }
}

/// Operands cached for one reported extent failure during central-directory recovery.
#[derive(Debug, Clone, Copy)]
pub struct ObservedRecoveryDataOutsideFile {
    local_header_offset: ObservedRecoveryWord,
    local_name_length: ObservedRecoveryWord,
    local_extra_length: ObservedRecoveryWord,
    compressed_size: ObservedRecoveryWord,
    data_start: u64,
    candidate_signature: MetadataRange,
    zip64: Option<ObservedRecoveryZip64Selection>,
}

impl ObservedRecoveryDataOutsideFile {
    pub(crate) fn read(
        entry: &Entry,
        local: &LocalStart,
        bounds: &ZipBoundWords,
    ) -> Result<Self, FormatError> {
        let central = entry.metadata().record_range();
        let start = central.offset();
        let bound = central.end();
        let zip64 = (bounds.has_size_marker || bounds.has_offset_marker)
            .then(|| bounds.selection(central))
            .transpose()?;
        Ok(Self {
            local_header_offset: word(
                entry.data_offset,
                bounds.offsets.local_header_offset,
                start + ZIP_LOCAL_OFFSET_AT,
                bound,
            )?,
            local_name_length: ObservedRecoveryWord {
                value: u64::from(local.name_len),
                range: MetadataRange::checked(entry.data_offset + LOCAL_NAME_AT as u64, 2, bound)?,
            },
            local_extra_length: ObservedRecoveryWord {
                value: u64::from(local.extra_len),
                range: MetadataRange::checked(entry.data_offset + LOCAL_EXTRA_AT as u64, 2, bound)?,
            },
            compressed_size: word(
                entry.compressed_size,
                bounds.offsets.compressed_size,
                start + ZIP_COMPRESSED_AT,
                bound,
            )?,
            data_start: local.data_start,
            candidate_signature: MetadataRange::checked(start, 4, bound)?,
            zip64,
        })
    }

    /// Returns the selected central word pointing to the matched local header.
    #[must_use]
    pub const fn local_header_offset(self) -> ObservedRecoveryWord {
        self.local_header_offset
    }

    /// Returns the name length read from the matched local header.
    #[must_use]
    pub const fn local_name_length(self) -> ObservedRecoveryWord {
        self.local_name_length
    }

    /// Returns the extra length read from the matched local header.
    #[must_use]
    pub const fn local_extra_length(self) -> ObservedRecoveryWord {
        self.local_extra_length
    }

    /// Returns the selected central compressed-size word.
    #[must_use]
    pub const fn compressed_size(self) -> ObservedRecoveryWord {
        self.compressed_size
    }

    /// Returns the computed start of the payload after the local header.
    #[must_use]
    pub const fn data_start(self) -> u64 {
        self.data_start
    }

    /// Returns the signature at the candidate boundary observed by recovery.
    #[must_use]
    pub const fn candidate_signature(self) -> MetadataRange {
        self.candidate_signature
    }

    /// Returns ZIP64 selection inputs when the extent used a fixed sentinel.
    #[must_use]
    pub const fn zip64_selection(self) -> Option<ObservedRecoveryZip64Selection> {
        self.zip64
    }
}

impl ZipBoundWords {
    fn selection(
        &self,
        central: MetadataRange,
    ) -> Result<ObservedRecoveryZip64Selection, FormatError> {
        let start = central.offset();
        let end = central.end();
        Ok(ObservedRecoveryZip64Selection {
            size_words: MetadataRange::checked(start + ZIP_COMPRESSED_AT, 8, end)?,
            extra_length_word: MetadataRange::checked(start + ZIP_EXTRA_LENGTH_AT, 2, end)?,
            local_offset_word: MetadataRange::checked(start + ZIP_LOCAL_OFFSET_AT, 4, end)?,
            header: self
                .offsets
                .header
                .map(|offset| MetadataRange::checked(offset, 4, end))
                .transpose()?,
            complete: self.offsets.header.is_some() || self.extra_len == 0,
        })
    }
}

fn word(
    value: u64,
    zip64: Option<u64>,
    fixed: u64,
    bound: u64,
) -> Result<ObservedRecoveryWord, FormatError> {
    let (offset, len) = zip64.map_or((fixed, 4), |offset| (offset, 8));
    Ok(ObservedRecoveryWord {
        value,
        range: MetadataRange::checked(offset, len, bound)?,
    })
}
