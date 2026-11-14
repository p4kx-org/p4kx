// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice::Iter;
use std::sync::{Arc, OnceLock};

use serde::{Serialize, Serializer};

use crate::source_use::SourceUseRef;

#[derive(Debug)]
pub(super) enum OriginRefs {
    Flat(Vec<SourceUseRef>),
    Shared(Box<SharedRefs>),
}

impl OriginRefs {
    pub(super) fn shared(basis: &Arc<[SourceUseRef]>, mut additions: Vec<SourceUseRef>) -> Self {
        additions.retain(|item| basis.binary_search(item).is_err());
        Self::Shared(Box::new(SharedRefs {
            basis: Arc::clone(basis),
            additions,
            expanded: OnceLock::new(),
        }))
    }

    pub(super) fn as_slice(&self) -> &[SourceUseRef] {
        match self {
            Self::Flat(refs) => refs,
            Self::Shared(refs) => refs.expanded.get_or_init(|| refs.iter().cloned().collect()),
        }
    }

    fn iter(&self) -> RefIter<'_> {
        match self {
            Self::Flat(refs) => RefIter::new(refs, &[]),
            Self::Shared(refs) => refs.iter(),
        }
    }

    #[cfg(test)]
    pub(super) fn is_shared_unexpanded(&self) -> bool {
        matches!(self, Self::Shared(refs) if refs.expanded.get().is_none())
    }
}

impl PartialEq for OriginRefs {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl Eq for OriginRefs {}

impl Serialize for OriginRefs {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Flat(refs) => refs.serialize(serializer),
            Self::Shared(refs) => serializer.collect_seq(refs.iter()),
        }
    }
}

#[derive(Debug)]
pub(super) struct SharedRefs {
    basis: Arc<[SourceUseRef]>,
    additions: Vec<SourceUseRef>,
    expanded: OnceLock<Vec<SourceUseRef>>,
}

impl SharedRefs {
    fn iter(&self) -> RefIter<'_> {
        RefIter::new(&self.basis, &self.additions)
    }
}

struct RefIter<'a> {
    basis: Iter<'a, SourceUseRef>,
    additions: Iter<'a, SourceUseRef>,
}

impl<'a> RefIter<'a> {
    fn new(basis: &'a [SourceUseRef], additions: &'a [SourceUseRef]) -> Self {
        Self {
            basis: basis.iter(),
            additions: additions.iter(),
        }
    }
}

impl<'a> Iterator for RefIter<'a> {
    type Item = &'a SourceUseRef;

    fn next(&mut self) -> Option<Self::Item> {
        match (
            self.basis.as_slice().first(),
            self.additions.as_slice().first(),
        ) {
            (Some(basis), Some(addition)) if addition < basis => self.additions.next(),
            (Some(_), _) => self.basis.next(),
            (None, _) => self.additions.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.basis.len() + self.additions.len();
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for RefIter<'_> {}
