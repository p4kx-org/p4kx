// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use crate::metadata_range::MetadataRange;
use crate::metadata_read_error::MetadataReadError;
use crate::source::Source;

/// A metadata range privately bound to one borrowed owning source.
#[derive(Clone, Copy)]
pub struct MetadataSpan<'source> {
    source: &'source Source,
    range: MetadataRange,
}

impl<'source> MetadataSpan<'source> {
    pub(crate) const fn bind(source: &'source Source, range: MetadataRange) -> Self {
        Self { source, range }
    }

    /// Returns the checked owning-source range.
    #[must_use]
    pub const fn range(self) -> MetadataRange {
        self.range
    }

    /// Restricts an existing bound span without granting access to unrelated source bytes.
    ///
    /// # Errors
    /// Rejects overflowing or out-of-span extents.
    pub fn subspan(self, offset: u64, len: u64) -> Result<Self, MetadataReadError> {
        if offset
            .checked_add(len)
            .is_none_or(|end| end > self.range.len())
        {
            return Err(MetadataReadError::OutsideSpan);
        }
        let start = self
            .range
            .offset()
            .checked_add(offset)
            .ok_or(MetadataReadError::OutsideSpan)?;
        let range = MetadataRange::checked(start, len, self.source.len())
            .map_err(|_| MetadataReadError::OutsideSpan)?;
        Ok(Self::bind(self.source, range))
    }

    pub(crate) fn belongs_to(self, source: &Source) -> bool {
        std::ptr::eq(self.source, source)
    }
}

impl fmt::Debug for MetadataSpan<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MetadataSpan")
            .field("range", &self.range)
            .finish_non_exhaustive()
    }
}
