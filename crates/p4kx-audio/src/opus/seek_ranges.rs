// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use super::{EmbeddedOpusHeaderPages, SwitchOpusPreSkip};

/// Original WEM-relative ranges used for a seekable Opus conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WemOpusRanges {
    /// Checked format prefix covering the original layout, counts and pre-skip selectors.
    pub format: Range<u64>,
    /// Selected packet size words from the standalone seek chunk, when used.
    pub seek_table: Option<Range<u64>>,
    /// Checked embedded Ogg pages containing the original identification and comment packets.
    pub embedded_headers: Option<Range<u64>>,
    /// Checked DATA bytes containing the audio packets and original framing.
    pub checked_data: Range<u64>,
    /// Checked prefix containing every emitted packet, including source framing or page bytes.
    pub emitted_data: Range<u64>,
    pub(super) embedded_header_pages: Option<EmbeddedOpusHeaderPages>,
    pub(super) switch_pre_skip: Option<SwitchOpusPreSkip>,
}

impl WemOpusRanges {
    /// Returns the original pages supplying embedded identification and comments.
    #[must_use]
    pub const fn embedded_header_pages(&self) -> Option<&EmbeddedOpusHeaderPages> {
        self.embedded_header_pages.as_ref()
    }

    /// Returns the first-packet inputs when they determine the Switch pre-skip.
    #[must_use]
    pub const fn switch_pre_skip(&self) -> Option<&SwitchOpusPreSkip> {
        self.switch_pre_skip.as_ref()
    }
}
