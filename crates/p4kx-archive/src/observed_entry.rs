// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::origin_ancestor::OriginAncestor;

use crate::{
    ArchiveInputOrigin, Entry, EntryOrigin, EntryOriginStep, ObservedEntryBinding, SourceEncryption,
};

/// Borrowed original Entry evidence without copying stored paths or declarations.
#[derive(Debug, Clone, Copy)]
pub struct ObservedEntry<'archive> {
    input: &'archive ArchiveInputOrigin,
    ancestors: &'archive Arc<[OriginAncestor]>,
    entry: &'archive Entry,
}

impl<'archive> ObservedEntry<'archive> {
    pub(crate) const fn new(
        input: &'archive ArchiveInputOrigin,
        ancestors: &'archive Arc<[OriginAncestor]>,
        entry: &'archive Entry,
    ) -> Self {
        Self {
            input,
            ancestors,
            entry,
        }
    }

    /// Returns the actual opaque root input supplying this Entry.
    #[must_use]
    pub const fn input_origin(&self) -> &ArchiveInputOrigin {
        self.input
    }

    /// Returns the exact borrowed Entry admitted by its owning Archive.
    #[must_use]
    pub const fn entry(&self) -> &Entry {
        self.entry
    }

    /// Iterates original containing Entries without copying their stored declarations.
    pub fn ancestors(&self) -> impl ExactSizeIterator<Item = Option<&EntryOriginStep>> {
        self.ancestors.iter().map(OriginAncestor::step)
    }

    /// Returns the original fixed-width admitted leaf ordinal.
    #[must_use]
    pub const fn entry_index(&self) -> u32 {
        self.entry.index
    }

    /// Returns the leaf's reconciled stored encryption declaration.
    #[must_use]
    pub fn stored_encryption(&self) -> SourceEncryption {
        SourceEncryption::of(self.entry.metadata())
    }

    /// Retains original ownership and coordinates without copying variable source details.
    #[must_use]
    pub fn binding(&self) -> ObservedEntryBinding {
        ObservedEntryBinding::new(
            self.input.clone(),
            Arc::clone(self.ancestors),
            self.entry.index,
        )
    }

    /// Copies full source details only when the caller explicitly retains them.
    #[must_use]
    pub fn snapshot(&self) -> EntryOrigin {
        EntryOrigin::new(
            self.input.clone(),
            Arc::clone(self.ancestors),
            Arc::new(EntryOriginStep::of(self.entry)),
        )
    }
}
