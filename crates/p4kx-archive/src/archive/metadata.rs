// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Archive;
use crate::archive_metadata::ArchiveMetadata;
use crate::metadata_view::MetadataView;

impl Archive {
    /// Returns source layout declarations without implying raw retention or verification.
    #[must_use]
    pub const fn metadata(&self) -> &ArchiveMetadata {
        &self.metadata
    }

    /// Borrows metadata-only positional access to this exact owning source.
    #[must_use]
    pub fn metadata_view(&self) -> MetadataView<'_> {
        MetadataView::new(&self.source, &self.metadata, &self.entries)
    }
}
