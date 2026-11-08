// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::{ArchiveInputOrigin, EntryOriginStep, SourceEncryption};

/// Owned Entry evidence bound to its observed root input and complete containing context.
#[derive(Debug, Clone)]
pub struct EntryOrigin {
    input: ArchiveInputOrigin,
    ancestors: Arc<[Option<Arc<EntryOriginStep>>]>,
    entry: Arc<EntryOriginStep>,
}

impl EntryOrigin {
    pub(crate) const fn new(
        input: ArchiveInputOrigin,
        ancestors: Arc<[Option<Arc<EntryOriginStep>>]>,
        entry: Arc<EntryOriginStep>,
    ) -> Self {
        Self {
            input,
            ancestors,
            entry,
        }
    }

    /// Returns the opaque root-input binding retained beyond the Archive lifetime.
    #[must_use]
    pub const fn input_origin(&self) -> &ArchiveInputOrigin {
        &self.input
    }

    /// Returns the actual admitted Entry supplying these bytes.
    #[must_use]
    pub fn entry(&self) -> &EntryOriginStep {
        &self.entry
    }

    /// Iterates containing Entries from the root to the immediate parent.
    /// A missing step records a container opened without proved Entry membership.
    pub fn ancestors(&self) -> impl ExactSizeIterator<Item = Option<&EntryOriginStep>> {
        self.ancestors.iter().map(Option::as_deref)
    }

    /// Combines actual containing and leaf Entry evidence without reading payloads.
    #[must_use]
    pub fn encryption(&self) -> SourceEncryption {
        self.ancestors()
            .map(|step| step.map_or(SourceEncryption::Unknown, EntryOriginStep::encryption))
            .fold(self.entry.encryption(), SourceEncryption::combine)
    }
}
