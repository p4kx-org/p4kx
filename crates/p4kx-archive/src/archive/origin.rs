// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;
use std::sync::Arc;

use crate::{Archive, ArchiveInputOrigin, Entry, EntryOrigin, EntryOriginStep};

impl Archive {
    /// Snapshots evidence for an exact borrowed Entry slot of this Archive.
    /// Foreign Entries and detached clones return None, including an equal numeric index.
    #[must_use]
    pub fn entry_origin(&self, entry: &Entry) -> Option<EntryOrigin> {
        self.origin_step(entry).map(|step| {
            EntryOrigin::new(
                self.input_origin.clone(),
                Arc::clone(&self.origin_ancestors),
                step,
            )
        })
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
                .all(|(owned, observed)| match (owned.as_deref(), observed) {
                    (Some(owned), Some(observed)) => owned.index() == observed.index(),
                    _ => false,
                })
    }

    pub(super) fn child_origins(&self, entry: &Entry) -> Arc<[Option<Arc<EntryOriginStep>>]> {
        let mut ancestors = Vec::with_capacity(self.origin_ancestors.len() + 1);
        ancestors.extend(self.origin_ancestors.iter().cloned());
        ancestors.push(self.origin_step(entry));
        ancestors.into()
    }

    fn origin_step(&self, entry: &Entry) -> Option<Arc<EntryOriginStep>> {
        let owned = self.entries.get(entry.index())?;
        ptr::eq(owned, entry).then(|| Arc::new(EntryOriginStep::of(entry)))
    }
}
