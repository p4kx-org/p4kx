// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::sync::Arc;

use crate::field_origin::OriginContribution;
use crate::source_inventory::PhysicalSourceRef;
use crate::source_packed_use::{PackedSourceUse, PackedUseTree};
use crate::source_use::{SourceUseDescription, SourceUseRef};

/// Sorted source-use rows indexed by their own immutable IDs.
pub(crate) struct RegisteredUses {
    storage: UseStorage,
    pending_compaction: Option<PendingCompaction>,
}

struct UseStorage {
    live: BTreeSet<RegisteredUse>,
    packed: PackedUseTree,
    packed_sources: BTreeSet<Arc<PhysicalSourceRef>>,
}

struct PendingCompaction {
    pending: Vec<SourceUseRef>,
    deferred: Vec<SourceUseRef>,
}

impl RegisteredUses {
    pub(crate) const fn new(compact_coordinates: bool) -> Self {
        Self {
            storage: UseStorage {
                live: BTreeSet::new(),
                packed: PackedUseTree::new(),
                packed_sources: BTreeSet::new(),
            },
            pending_compaction: if compact_coordinates {
                Some(PendingCompaction {
                    pending: Vec::new(),
                    deferred: Vec::new(),
                })
            } else {
                None
            },
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.storage.live.len() + self.storage.packed.len()
    }

    pub(crate) fn packed_len(&self) -> usize {
        self.storage.packed.len()
    }

    pub(crate) fn get(&self, id: &SourceUseRef) -> Option<&Arc<SourceUseDescription>> {
        self.storage
            .live
            .get(id.as_str())
            .map(|registered| &registered.0)
    }

    pub(crate) fn contribution(&self, id: &SourceUseRef) -> Option<OriginContribution<'_>> {
        if let Some(live) = self.get(id) {
            return Some(OriginContribution::from(live.as_ref()));
        }
        let (id, packed) = self.storage.packed.get_key_value(id.as_str())?;
        Some(packed.contribution(id))
    }

    pub(crate) fn get_or_insert_with(
        &mut self,
        id: SourceUseRef,
        build: impl FnOnce(SourceUseRef) -> SourceUseDescription,
    ) -> Arc<SourceUseDescription> {
        if let Some(registered) = self.get(&id) {
            return Arc::clone(registered);
        }
        let row = match self.storage.packed.remove_entry(id.as_str()) {
            Some((original, packed)) => packed.restore(&original),
            None => build(id),
        };
        if let Some(pending) = &mut self.pending_compaction
            && row.location.pack_dataforge().is_some()
        {
            pending.pending.push(row.id().clone());
        }
        let row = Arc::new(row);
        self.storage.live.insert(RegisteredUse(Arc::clone(&row)));
        row
    }

    pub(crate) fn compact_batch<E>(
        &mut self,
        check: impl FnMut() -> Result<(), E>,
    ) -> Result<(), E> {
        let Some(PendingCompaction { pending, deferred }) = &mut self.pending_compaction else {
            return Ok(());
        };
        self.storage
            .compact_uses(pending, |id| deferred.push(id), check)
    }

    pub(crate) fn compact_pending<E>(
        &mut self,
        mut check: impl FnMut() -> Result<(), E>,
    ) -> Result<(), E> {
        let Some(queue) = &mut self.pending_compaction else {
            return Ok(());
        };
        self.storage
            .compact_uses(&mut queue.pending, drop, &mut check)?;
        self.storage
            .compact_uses(&mut queue.deferred, drop, check)?;
        queue.deferred = Vec::new();
        Ok(())
    }

    pub(crate) fn values(&self) -> impl ExactSizeIterator<Item = &Arc<SourceUseDescription>> {
        self.storage.live.iter().map(|registered| &registered.0)
    }

    pub(crate) fn packed_values(
        &self,
    ) -> impl ExactSizeIterator<Item = (&Arc<str>, &PackedSourceUse)> {
        self.storage.packed.iter()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        impl ExactSizeIterator<Item = Arc<SourceUseDescription>>,
        PackedUseTree,
    ) {
        (
            self.storage.live.into_iter().map(|registered| registered.0),
            self.storage.packed,
        )
    }
}

impl UseStorage {
    fn compact_uses<E>(
        &mut self,
        pending: &mut Vec<SourceUseRef>,
        mut held: impl FnMut(SourceUseRef),
        mut check: impl FnMut() -> Result<(), E>,
    ) -> Result<(), E> {
        while let Some(id) = pending.pop() {
            if let Err(error) = check() {
                pending.push(id);
                return Err(error);
            }
            let Some(registered) = self.live.get(id.as_str()) else {
                continue;
            };
            // A held use must keep the allocation that proves its registration.
            if Arc::strong_count(&registered.0) != 1 {
                held(id);
                continue;
            }
            if let Some(text) = registered.0.id().registered_text()
                && let Some(row) = PackedSourceUse::pack(&registered.0, &mut self.packed_sources)
            {
                let text = Arc::clone(text);
                self.live.remove(id.as_str());
                self.packed.insert(text, row);
            }
        }
        Ok(())
    }
}

impl Default for RegisteredUses {
    fn default() -> Self {
        Self::new(false)
    }
}

struct RegisteredUse(Arc<SourceUseDescription>);

impl Borrow<str> for RegisteredUse {
    fn borrow(&self) -> &str {
        self.0.id().as_str()
    }
}

impl PartialEq for RegisteredUse {
    fn eq(&self, other: &Self) -> bool {
        self.0.id() == other.0.id()
    }
}

impl Eq for RegisteredUse {}

impl PartialOrd for RegisteredUse {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RegisteredUse {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.id().cmp(other.0.id())
    }
}
