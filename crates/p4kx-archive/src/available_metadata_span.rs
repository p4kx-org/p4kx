// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::metadata_region::MetadataRegionStatus;
use crate::metadata_span::MetadataSpan;

/// Available metadata bytes with the explicit status of their full declaration.
#[derive(Debug, Clone, Copy)]
pub struct AvailableMetadataSpan<'source> {
    pub(crate) span: MetadataSpan<'source>,
    pub(crate) status: MetadataRegionStatus,
}

impl<'source> AvailableMetadataSpan<'source> {
    /// Returns the bounded available prefix, including an explicitly empty prefix.
    #[must_use]
    pub const fn span(self) -> MetadataSpan<'source> {
        self.span
    }

    /// Returns whether the complete declaration fits the owning source.
    #[must_use]
    pub const fn status(self) -> MetadataRegionStatus {
        self.status
    }
}
