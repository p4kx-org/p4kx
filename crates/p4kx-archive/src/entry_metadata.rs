// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::dos_timestamp::DosTimestamp;
use crate::entry_record_format::EntryRecordFormat;
use crate::metadata_range::MetadataRange;
use crate::zip_header_metadata::ZipHeaderMetadata;

/// Read-only source metadata from the Entry's directory record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMetadata {
    dos_timestamp: DosTimestamp,
    original_directory_ordinal: Option<u64>,
    raw_compression_method: u16,
    header: RecordHeader,
    record: MetadataRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordHeader {
    ZipCentralDirectory(ZipHeaderMetadata),
    P4kV2 { encryption: u16 },
}

impl EntryMetadata {
    pub(crate) const fn from_zip(
        dos_timestamp: DosTimestamp,
        raw_compression_method: u16,
        original_directory_ordinal: Option<u64>,
        header: ZipHeaderMetadata,
        record: MetadataRange,
    ) -> Self {
        Self {
            dos_timestamp,
            original_directory_ordinal,
            raw_compression_method,
            header: RecordHeader::ZipCentralDirectory(header),
            record,
        }
    }

    pub(crate) const fn from_v2(
        dos_timestamp: DosTimestamp,
        raw_compression_method: u16,
        original_directory_ordinal: u64,
        encryption: u16,
        record: MetadataRange,
    ) -> Self {
        Self {
            dos_timestamp,
            original_directory_ordinal: Some(original_directory_ordinal),
            raw_compression_method,
            header: RecordHeader::P4kV2 { encryption },
            record,
        }
    }

    /// Returns both stored DOS words; zero values remain present.
    #[must_use]
    pub const fn dos_timestamp(&self) -> DosTimestamp {
        self.dos_timestamp
    }

    /// Returns original directory-record coordinates, without a raw-retention claim.
    #[must_use]
    pub const fn record_range(&self) -> MetadataRange {
        self.record
    }

    /// Returns the physical directory-record format supplying these facts.
    #[must_use]
    pub const fn record_format(&self) -> EntryRecordFormat {
        match self.header {
            RecordHeader::ZipCentralDirectory(_) => EntryRecordFormat::ZipCentralDirectory,
            RecordHeader::P4kV2 { .. } => EntryRecordFormat::P4kV2,
        }
    }

    /// Returns the original zero-based directory position, unavailable for recovery-scanned records.
    /// The position precedes container folder removal and admitted-Entry reindexing.
    #[must_use]
    pub const fn original_directory_ordinal(&self) -> Option<u64> {
        self.original_directory_ordinal
    }

    /// Returns the complete source method word, independently of decoder selection.
    #[must_use]
    pub const fn raw_compression_method(&self) -> u16 {
        self.raw_compression_method
    }

    /// Returns original ZIP header words, absent for a v2 fixed record.
    #[must_use]
    pub const fn zip_header(&self) -> Option<&ZipHeaderMetadata> {
        match &self.header {
            RecordHeader::ZipCentralDirectory(header) => Some(header),
            RecordHeader::P4kV2 { .. } => None,
        }
    }

    /// Returns the stored v2 encryption word, absent for a ZIP directory record.
    #[must_use]
    pub const fn raw_v2_encryption_word(&self) -> Option<u16> {
        match self.header {
            RecordHeader::ZipCentralDirectory(_) => None,
            RecordHeader::P4kV2 { encryption } => Some(encryption),
        }
    }
}
