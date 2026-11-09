// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;
use std::sync::Arc;

use crate::origin_ancestor::OriginAncestor;

use crate::{Archive, ArchiveInputOrigin, Entry, EntryOrigin, EntryOriginStep, ObservedEntry};

impl Archive {
    /// Borrows evidence for an exact admitted Entry without copying variable source details.
    /// Foreign Entries and detached clones return None.
    #[must_use]
    pub fn observed_entry<'archive>(
        &'archive self,
        entry: &'archive Entry,
    ) -> Option<ObservedEntry<'archive>> {
        let owned = self.entries.get(entry.index())?;
        ptr::eq(owned, entry)
            .then(|| ObservedEntry::new(&self.input_origin, &self.origin_ancestors, entry))
    }

    /// Snapshots evidence for an exact borrowed Entry slot of this Archive.
    /// Foreign Entries and detached clones return None, including an equal numeric index.
    #[must_use]
    pub fn entry_origin(&self, entry: &Entry) -> Option<EntryOrigin> {
        self.observed_entry(entry).map(|view| view.snapshot())
    }

    /// Returns this view's opaque root-input identity, shared with all its containers.
    #[must_use]
    pub const fn input_origin(&self) -> &ArchiveInputOrigin {
        &self.input_origin
    }

    /// Returns whether evidence belongs to this exact root input and container position.
    /// Unknown ancestry cannot establish an owning view.
    #[must_use]
    pub fn owns_origin(&self, origin: &EntryOrigin) -> bool {
        self.input_origin.is_same_input(origin.input_origin())
            && self.entries.get(origin.entry().index()).is_some()
            && self.origin_ancestors.len() == origin.ancestors().len()
            && self
                .origin_ancestors
                .iter()
                .zip(origin.ancestors())
                .all(|(owned, observed)| match (owned.step(), observed) {
                    (Some(owned), Some(observed)) => owned.index() == observed.index(),
                    _ => false,
                })
    }

    pub(super) fn child_origins(&self, entry: &Entry) -> Arc<[OriginAncestor]> {
        let mut ancestors = Vec::with_capacity(self.origin_ancestors.len() + 1);
        ancestors.extend(self.origin_ancestors.iter().cloned());
        ancestors.push(
            self.origin_step(entry)
                .map_or_else(OriginAncestor::unproved, OriginAncestor::Proven),
        );
        ancestors.into()
    }

    fn origin_step(&self, entry: &Entry) -> Option<Arc<EntryOriginStep>> {
        self.observed_entry(entry)
            .map(|view| Arc::new(EntryOriginStep::of(view.entry())))
    }
}
