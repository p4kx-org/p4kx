// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::field_origin::OriginContribution;
use crate::source_inventory::PhysicalSourceRef;
use crate::source_packed_use::PackedSourceUse;
use crate::source_use::{SourceUseDescription, SourceUseRef};

/// Sorted source-use rows indexed by their own immutable IDs.
pub(crate) struct RegisteredUses {
    live: BTreeSet<RegisteredUse>,
    packed: BTreeMap<Arc<str>, PackedSourceUse>,
    packed_sources: BTreeSet<Arc<PhysicalSourceRef>>,
    pending_compaction: Option<Vec<SourceUseRef>>,
}

impl RegisteredUses {
    pub(crate) const fn new(compact_coordinates: bool) -> Self {
        Self {
            live: BTreeSet::new(),
            packed: BTreeMap::new(),
            packed_sources: BTreeSet::new(),
            pending_compaction: if compact_coordinates {
                Some(Vec::new())
            } else {
                None
            },
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.live.len() + self.packed.len()
    }

    pub(crate) fn packed_len(&self) -> usize {
        self.packed.len()
    }

    pub(crate) fn get(&self, id: &SourceUseRef) -> Option<&Arc<SourceUseDescription>> {
        self.live.get(id.as_str()).map(|registered| &registered.0)
    }

    pub(crate) fn contribution(&self, id: &SourceUseRef) -> Option<OriginContribution<'_>> {
        if let Some(live) = self.get(id) {
            return Some(OriginContribution::from(live.as_ref()));
        }
        let (id, packed) = self.packed.get_key_value(id.as_str())?;
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
        let row = match self.packed.remove_entry(id.as_str()) {
            Some((original, packed)) => packed.restore(&original),
            None => build(id),
        };
        if let Some(pending) = &mut self.pending_compaction
            && row.location.pack_dataforge().is_some()
        {
            pending.push(row.id().clone());
        }
        let row = Arc::new(row);
        self.live.insert(RegisteredUse(Arc::clone(&row)));
        row
    }

    pub(crate) fn compact_pending<E>(
        &mut self,
        mut check: impl FnMut() -> Result<(), E>,
    ) -> Result<(), E> {
        let Some(pending) = &mut self.pending_compaction else {
            return Ok(());
        };
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
                continue;
            }
            if let Some(text) = registered.0.id().registered_text()
                && let Some(packed) = PackedSourceUse::pack(&registered.0, &mut self.packed_sources)
            {
                let text = Arc::clone(text);
                self.live.remove(id.as_str());
                self.packed.insert(text, packed);
            }
        }
        Ok(())
    }

    pub(crate) fn values(&self) -> impl ExactSizeIterator<Item = &Arc<SourceUseDescription>> {
        self.live.iter().map(|registered| &registered.0)
    }

    pub(crate) fn packed_values(
        &self,
    ) -> impl ExactSizeIterator<Item = (&Arc<str>, &PackedSourceUse)> {
        self.packed.iter()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        impl ExactSizeIterator<Item = Arc<SourceUseDescription>>,
        impl ExactSizeIterator<Item = (Arc<str>, PackedSourceUse)>,
    ) {
        (
            self.live.into_iter().map(|registered| registered.0),
            self.packed.into_iter(),
        )
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
