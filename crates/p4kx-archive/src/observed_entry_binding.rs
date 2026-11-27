// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::origin_ancestor::OriginAncestor;

use crate::ArchiveInputOrigin;

/// Original runtime ownership of an Entry without copied paths or declaration payloads.
/// Equality and hashing follow [`Self::is_same_entry`].
#[derive(Debug, Clone)]
pub struct ObservedEntryBinding {
    input: ArchiveInputOrigin,
    ancestors: Arc<[OriginAncestor]>,
    depth: usize,
    index: u32,
}

impl ObservedEntryBinding {
    pub(crate) fn new(
        input: ArchiveInputOrigin,
        ancestors: Arc<[OriginAncestor]>,
        index: u32,
    ) -> Self {
        Self {
            input,
            depth: ancestors.len(),
            ancestors,
            index,
        }
    }

    /// Returns the actual opaque root-input owner.
    #[must_use]
    pub const fn input_origin(&self) -> &ArchiveInputOrigin {
        &self.input
    }

    /// Returns the original admitted leaf ordinal.
    #[must_use]
    pub const fn entry_index(&self) -> u32 {
        self.index
    }

    /// Returns the number of original containing Entry positions.
    #[must_use]
    pub const fn ancestor_count(&self) -> usize {
        self.depth
    }

    /// Returns whether the original containing context has unproved membership.
    #[must_use]
    pub fn has_unknown_ancestry(&self) -> bool {
        self.ancestors
            .iter()
            .take(self.depth)
            .any(|ancestor| ancestor.step().is_none())
    }

    /// Compares original owners and containing coordinates, including unproved-view identity.
    #[must_use]
    pub fn is_same_view(&self, other: &Self) -> bool {
        if !self.input.is_same_input(&other.input) || self.depth != other.depth {
            return false;
        }
        self.ancestors
            .iter()
            .take(self.depth)
            .zip(other.ancestors.iter().take(other.depth))
            .all(|(left, right)| left.is_same(right))
    }

    /// Returns whether both handles identify the same original Entry occurrence.
    #[must_use]
    pub fn is_same_entry(&self, other: &Self) -> bool {
        self.index == other.index && self.is_same_view(other)
    }

    /// Returns an original containing Entry without copying its variable metadata.
    #[must_use]
    pub fn ancestor(&self, position: usize) -> Option<Self> {
        if position >= self.depth {
            return None;
        }
        let step = self.ancestors.get(position)?.step()?;
        Some(Self {
            input: self.input.clone(),
            ancestors: Arc::clone(&self.ancestors),
            depth: position,
            index: step.ordinal(),
        })
    }
}

impl PartialEq for ObservedEntryBinding {
    fn eq(&self, other: &Self) -> bool {
        self.is_same_entry(other)
    }
}

impl Eq for ObservedEntryBinding {}

impl Hash for ObservedEntryBinding {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.input.hash_identity(state);
        self.depth.hash(state);
        for ancestor in self.ancestors.iter().take(self.depth) {
            ancestor.hash_context(state);
        }
        self.index.hash(state);
    }
}
