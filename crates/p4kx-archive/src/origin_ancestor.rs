// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::{Hash, Hasher};
use std::mem;
use std::ptr;
use std::sync::Arc;

use crate::EntryOriginStep;

#[derive(Debug, Clone)]
pub(crate) enum OriginAncestor {
    Proven(Arc<EntryOriginStep>),
    Unproved(Arc<()>),
}

impl OriginAncestor {
    pub(crate) fn unproved() -> Self {
        Self::Unproved(Arc::new(()))
    }

    pub(crate) fn step(&self) -> Option<&EntryOriginStep> {
        match self {
            Self::Proven(step) => Some(step),
            Self::Unproved(_) => None,
        }
    }

    pub(crate) fn is_same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Proven(left), Self::Proven(right)) => left.index() == right.index(),
            (Self::Unproved(left), Self::Unproved(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    pub(crate) fn hash_context<H: Hasher>(&self, state: &mut H) {
        mem::discriminant(self).hash(state);
        match self {
            Self::Proven(step) => step.ordinal().hash(state),
            Self::Unproved(identity) => ptr::hash(Arc::as_ptr(identity), state),
        }
    }
}
