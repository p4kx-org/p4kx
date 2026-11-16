// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::sync::Arc;

use crate::source_use::{SourceUseDescription, SourceUseRef};

/// Sorted source-use rows indexed by their own immutable IDs.
pub(crate) struct RegisteredUses(BTreeSet<RegisteredUse>);

impl RegisteredUses {
    pub(crate) const fn new() -> Self {
        Self(BTreeSet::new())
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn get(&self, id: &SourceUseRef) -> Option<&Arc<SourceUseDescription>> {
        self.0.get(id.as_str()).map(|registered| &registered.0)
    }

    pub(crate) fn get_or_insert_with(
        &mut self,
        id: SourceUseRef,
        build: impl FnOnce(SourceUseRef) -> SourceUseDescription,
    ) -> Arc<SourceUseDescription> {
        if let Some(registered) = self.get(&id) {
            return Arc::clone(registered);
        }
        let row = Arc::new(build(id));
        self.0.insert(RegisteredUse(Arc::clone(&row)));
        row
    }

    pub(crate) fn values(&self) -> impl ExactSizeIterator<Item = &Arc<SourceUseDescription>> {
        self.0.iter().map(|registered| &registered.0)
    }

    pub(crate) fn into_values(self) -> impl ExactSizeIterator<Item = Arc<SourceUseDescription>> {
        self.0.into_iter().map(|registered| registered.0)
    }
}

impl Default for RegisteredUses {
    fn default() -> Self {
        Self::new()
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
