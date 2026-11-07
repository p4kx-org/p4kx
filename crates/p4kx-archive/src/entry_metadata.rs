// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::dos_timestamp::DosTimestamp;

/// Read-only source metadata from the Entry's directory record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMetadata {
    dos_timestamp: DosTimestamp,
}

impl EntryMetadata {
    pub(crate) const fn new(dos_timestamp: DosTimestamp) -> Self {
        Self { dos_timestamp }
    }

    /// Returns both stored DOS words; zero values remain present.
    #[must_use]
    pub const fn dos_timestamp(&self) -> DosTimestamp {
        self.dos_timestamp
    }
}
