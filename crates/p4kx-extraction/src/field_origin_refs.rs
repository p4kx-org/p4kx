// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice::{Iter, from_ref};
use std::sync::{Arc, OnceLock};

use serde::{Serialize, Serializer};

use crate::source_use::{SourceUse, SourceUseRef};

use super::{FieldOrigin, OriginFacts, SharedFieldOrigin};

#[derive(Debug)]
pub(super) enum OriginRefs {
    Single(SourceUseRef),
    Flat(Vec<SourceUseRef>),
    Basis(Arc<[SourceUseRef]>),
    Shared(Box<SharedRefs>),
}

impl OriginRefs {
    pub(super) fn flat(mut references: Vec<SourceUseRef>) -> Self {
        if references.len() == 1
            && let Some(reference) = references.pop()
        {
            Self::Single(reference)
        } else {
            Self::Flat(references)
        }
    }

    pub(super) fn shared(basis: &Arc<[SourceUseRef]>, additions: Vec<SourceUseRef>) -> Self {
        if basis.is_empty() {
            Self::flat(additions)
        } else if additions.is_empty() {
            Self::Basis(Arc::clone(basis))
        } else {
            Self::Shared(Box::new(SharedRefs {
                basis: Arc::clone(basis),
                additions,
                expanded: OnceLock::new(),
            }))
        }
    }

    pub(super) fn as_slice(&self) -> &[SourceUseRef] {
        match self {
            Self::Single(reference) => from_ref(reference),
            Self::Flat(refs) => refs,
            Self::Basis(refs) => refs,
            Self::Shared(refs) => refs.expanded.get_or_init(|| refs.iter().cloned().collect()),
        }
    }

    pub(super) fn iter(&self) -> RefIter<'_> {
        match self {
            Self::Shared(refs) => refs.iter(),
            _ => RefIter::new(self.as_slice(), &[]),
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
            Self::Shared(refs) => serializer.collect_seq(refs.iter()),
            _ => self.as_slice().serialize(serializer),
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
    pub(super) fn iter(&self) -> RefIter<'_> {
        RefIter::new(&self.basis, &self.additions)
    }
}

pub(super) struct RefIter<'a> {
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

impl FieldOrigin {
    pub(crate) fn with_shared<'a>(
        shared: &SharedFieldOrigin,
        uses: impl IntoIterator<Item = &'a SourceUse>,
        complete: bool,
    ) -> Self {
        let facts = OriginFacts::measure(
            uses.into_iter().map(SourceUse::description),
            complete,
            shared.mode,
        );
        Self::from_shared_facts(shared, facts)
    }

    pub(super) fn from_shared_facts(shared: &SharedFieldOrigin, facts: OriginFacts) -> Self {
        Self {
            identity: Some(Arc::clone(&shared.identity)),
            fields: shared.fields_with(facts),
        }
    }
}
