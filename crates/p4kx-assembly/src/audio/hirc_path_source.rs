// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_audio::HircObject;

use super::walk_banks::Loaded;

/// The original bank and bank-relative byte spans of one resolved HIRC object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HircPathSource {
    entry_position: usize,
    object: Range<usize>,
    id: Range<usize>,
    version: Option<Range<usize>>,
}

impl HircPathSource {
    pub(super) fn of(owner: &Loaded<'_>, object: &HircObject) -> Self {
        Self {
            entry_position: owner.soundbank.entry_position(),
            object: object.byte_range(),
            id: object.id_byte_range(),
            version: owner.bank.version_byte_range(),
        }
    }

    /// Returns the bank's physical Entry position in the Archive.
    #[must_use]
    pub const fn entry_position(&self) -> usize {
        self.entry_position
    }

    /// Returns the half-open extent of the object within the decoded bank bytes.
    #[must_use]
    pub fn object_bytes(&self) -> Range<usize> {
        self.object.clone()
    }

    /// Returns the half-open extent of the object ID.
    #[must_use]
    pub fn id_bytes(&self) -> Range<usize> {
        self.id.clone()
    }

    /// Returns the half-open extent of the bank-version selector.
    #[must_use]
    pub fn version_bytes(&self) -> Option<Range<usize>> {
        self.version.clone()
    }
}

#[derive(Clone, Copy)]
pub(super) enum PathSourceMode {
    Omit,
    Retain,
}
