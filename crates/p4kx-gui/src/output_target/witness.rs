// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::any::Any;
use std::sync::{Arc, Weak};

/// A weak in-process owner identity with its existing result revisions.
#[derive(Debug, Clone)]
pub(crate) struct Witness {
    owner: Weak<dyn Any + Send + Sync>,
    generation: u64,
    revision: u64,
}

impl Witness {
    pub(crate) fn of<T: Any + Send + Sync>(owner: &Arc<T>) -> Self {
        let erased: Arc<dyn Any + Send + Sync> = Arc::<T>::clone(owner);
        Self {
            owner: Arc::downgrade(&erased),
            generation: 0,
            revision: 0,
        }
    }

    pub(crate) const fn at(mut self, generation: u64, revision: u64) -> Self {
        self.generation = generation;
        self.revision = revision;
        self
    }
}

impl PartialEq for Witness {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation
            && self.revision == other.revision
            && self.owner.ptr_eq(&other.owner)
    }
}

impl Eq for Witness {}

/// A setup lifetime that is replaced when its setup is recreated.
#[derive(Debug, Clone, Default)]
pub(crate) struct OwnerLease(Arc<()>);

impl OwnerLease {
    pub(crate) fn witness(&self) -> Witness {
        Witness::of(&self.0)
    }
}
