// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{DosTimestamp, Entry};
use schemars::JsonSchema;
use serde::Serialize;

use crate::decimal_u64::DecimalU64;
use crate::entry_record_format_info::EntryRecordFormatInfo;
use crate::zip_header_info::ZipHeaderInfo;

/// Shared encoded directory metadata of one physical admitted Entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[schemars(extend("required" = [
    "owner_entry_index", "dos_timestamp", "record_format", "original_directory_ordinal",
    "raw_compression_method", "zip_header", "raw_v2_encryption_word"
]))]
#[schemars(extend("allOf" = [
    {
        "if": {"properties": {"record_format": {"const": "zip_central_directory"}}},
        "then": {"properties": {"zip_header": {"type": "object"}, "raw_v2_encryption_word": {"type": "null"}}}
    },
    {
        "if": {"properties": {"record_format": {"const": "p4k_v2"}}},
        "then": {"properties": {"zip_header": {"type": "null"}, "raw_v2_encryption_word": {"type": "integer"}}}
    }
]))]
#[non_exhaustive]
pub struct EntryMetadataInfo {
    /// Index in the immediately owning Archive, not a grouped texture's aggregate identity.
    pub owner_entry_index: usize,
    /// Exact source words and their separate calendar interpretation.
    pub dos_timestamp: DosTimestampInfo,
    /// Physical directory-record format, independent of decoder selection.
    pub record_format: EntryRecordFormatInfo,
    /// Original directory position before admission, unavailable for recovery-scanned headers.
    pub original_directory_ordinal: Option<DecimalU64>,
    /// Unmodified unsigned compression-method word.
    pub raw_compression_method: u16,
    /// Fixed ZIP words, explicitly absent on v2 records.
    pub zip_header: Option<ZipHeaderInfo>,
    /// Raw v2 encryption word, explicitly absent on ZIP records.
    pub raw_v2_encryption_word: Option<u16>,
}

impl EntryMetadataInfo {
    /// Describes the directory record owned by this exact physical Entry.
    #[must_use]
    pub const fn of(entry: &Entry) -> Self {
        let metadata = entry.metadata();
        Self {
            owner_entry_index: entry.index(),
            dos_timestamp: DosTimestampInfo::of(metadata.dos_timestamp()),
            record_format: EntryRecordFormatInfo::of(metadata.record_format()),
            original_directory_ordinal: match metadata.original_directory_ordinal() {
                Some(ordinal) => Some(DecimalU64::new(ordinal)),
                None => None,
            },
            raw_compression_method: metadata.raw_compression_method(),
            zip_header: match metadata.zip_header() {
                Some(header) => Some(ZipHeaderInfo::of(header)),
                None => None,
            },
            raw_v2_encryption_word: metadata.raw_v2_encryption_word(),
        }
    }
}

/// Stored DOS date/time words with no timezone conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct DosTimestampInfo {
    /// Unmodified unsigned 16-bit date word, including zero and invalid encodings.
    pub raw_date: u16,
    /// Unmodified unsigned 16-bit time word, including zero and invalid encodings.
    pub raw_time: u16,
    /// Valid calendar fields or an explicit invalid interpretation; no zone is implied.
    pub calendar: DosCalendarInfo,
}

impl DosTimestampInfo {
    const fn of(timestamp: DosTimestamp) -> Self {
        let calendar = match timestamp.calendar() {
            Some(value) => DosCalendarInfo::Valid {
                year: value.year(),
                month: value.month(),
                day: value.day(),
                hour: value.hour(),
                minute: value.minute(),
                second: value.second(),
            },
            None => DosCalendarInfo::Invalid,
        };
        Self {
            raw_date: timestamp.raw_date(),
            raw_time: timestamp.raw_time(),
            calendar,
        }
    }
}

/// Calendar interpretation of stored DOS words, independent of integrity verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
#[non_exhaustive]
pub enum DosCalendarInfo {
    /// The source words do not encode a valid calendar date and time.
    Invalid,
    /// Valid calendar fields without a known timezone.
    Valid {
        /// Calendar year.
        #[schemars(range(min = 1980, max = 2107))]
        year: u16,
        /// Calendar month.
        #[schemars(range(min = 1, max = 12))]
        month: u8,
        /// Calendar day, validated against the stated month and year.
        #[schemars(range(min = 1, max = 31))]
        day: u8,
        /// Hour of the stated calendar day.
        #[schemars(range(min = 0, max = 23))]
        hour: u8,
        /// Minute of the stated hour.
        #[schemars(range(min = 0, max = 59))]
        minute: u8,
        /// Even second from the DOS two-second encoding.
        #[schemars(range(min = 0, max = 58), extend("multipleOf" = 2))]
        second: u8,
    },
}
