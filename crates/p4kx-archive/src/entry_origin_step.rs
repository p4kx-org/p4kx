// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Entry, EntryMetadata, SourceEncryption};

/// Owned declaration evidence for one admitted Entry in its immediately owning Archive.
#[derive(Debug)]
pub struct EntryOriginStep {
    index: u32,
    path: String,
    metadata: EntryMetadata,
}

impl EntryOriginStep {
    pub(crate) fn of(entry: &Entry) -> Self {
        Self {
            index: entry.index,
            path: entry.path().to_owned(),
            metadata: entry.metadata().clone(),
        }
    }

    /// Returns the admitted Entry index within its immediately owning Archive.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index as usize
    }

    /// Returns the original fixed-width admitted Entry ordinal.
    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.index
    }

    /// Returns the stored Entry path without a machine host path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the retained directory-record declarations and owning-source coordinates.
    #[must_use]
    pub const fn metadata(&self) -> &EntryMetadata {
        &self.metadata
    }

    /// Reconciles supported CIG/v2 evidence with separate ZIP encryption flags.
    #[must_use]
    pub fn encryption(&self) -> SourceEncryption {
        SourceEncryption::of(&self.metadata)
    }
}
