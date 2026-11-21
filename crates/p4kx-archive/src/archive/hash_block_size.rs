// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::format_error::FormatError;
use crate::{Archive, EntryIssue, MetadataRange};

const WORD_BYTES: u64 = size_of::<u16>() as u64;

/// The stored ZIP extra header words that supply a hash-block size issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservedHashBlockSize {
    id: MetadataRange,
    declared_len: MetadataRange,
}

impl ObservedHashBlockSize {
    pub(crate) fn at(start: u64, end: u64) -> Result<Self, FormatError> {
        let id = MetadataRange::checked(start, WORD_BYTES, end)?;
        let declared_len = MetadataRange::checked(id.end(), WORD_BYTES, end)?;
        Ok(Self { id, declared_len })
    }

    /// Returns the ZIP extra block's type word range.
    #[must_use]
    pub const fn id(self) -> MetadataRange {
        self.id
    }

    /// Returns the original length word range, including no digest bytes.
    #[must_use]
    pub const fn declared_len(self) -> MetadataRange {
        self.declared_len
    }
}

impl Archive {
    /// Returns the cached header for a retained hash-block size issue.
    #[must_use]
    pub fn observed_hash_block_size(&self, issue_position: usize) -> Option<ObservedHashBlockSize> {
        let (index, EntryIssue::HashBlockSize { .. }) = self.issues().get(issue_position)? else {
            return None;
        };
        self.log.hash_block_size(*index)
    }
}
