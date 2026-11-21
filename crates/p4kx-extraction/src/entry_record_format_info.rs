// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::EntryRecordFormat;
use schemars::JsonSchema;
use serde::Serialize;

/// Directory-record format reported by the owning Archive reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EntryRecordFormatInfo {
    /// A ZIP central-directory record, including P4K v1.
    ZipCentralDirectory,
    /// A fixed P4K v2 directory record.
    P4kV2,
    /// A reader format whose interpretation this projection does not know.
    Unknown,
}

impl EntryRecordFormatInfo {
    pub(crate) const fn of(format: EntryRecordFormat) -> Self {
        match format {
            EntryRecordFormat::ZipCentralDirectory => Self::ZipCentralDirectory,
            EntryRecordFormat::P4kV2 => Self::P4kV2,
            _ => Self::Unknown,
        }
    }
}
