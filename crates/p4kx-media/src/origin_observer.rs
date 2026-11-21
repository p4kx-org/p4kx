// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, ArchiveInputOrigin, Entry, EntryOrigin, SourceUses};

use crate::origin_input::MediaOriginInput;
use crate::{MediaError, origin_error};

/// Original registration authority detached from a writer borrow, sharing its single inventory.
#[derive(Clone)]
pub(crate) struct OriginObserver {
    pub(crate) owner: SourceUses,
    pub(crate) root: ArchiveInputOrigin,
    pub(crate) detail_bytes: u64,
}

impl OriginObserver {
    pub(crate) fn entry(
        &self,
        archive: &Archive,
        entry: &Entry,
    ) -> Result<MediaOriginInput, MediaError> {
        if !self.root.is_same_input(archive.input_origin()) {
            return Err(origin_error::foreign());
        }
        MediaOriginInput::observe(&self.owner, archive, entry, self.detail_bytes)
    }

    pub(crate) fn original(&self, origin: &EntryOrigin) -> Result<MediaOriginInput, MediaError> {
        if !self.root.is_same_input(origin.input_origin()) {
            return Err(origin_error::foreign());
        }
        MediaOriginInput::observe_origin(&self.owner, origin, self.detail_bytes)
    }
}
