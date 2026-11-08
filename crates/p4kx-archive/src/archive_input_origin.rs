// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

/// Opaque identity of one observed root Archive input, shared by its containers.
#[derive(Debug, Clone)]
pub struct ArchiveInputOrigin {
    identity: Arc<()>,
}

impl ArchiveInputOrigin {
    pub(crate) fn new() -> Self {
        Self {
            identity: Arc::new(()),
        }
    }

    /// Returns whether both handles belong to the same observed root Archive input.
    #[must_use]
    pub fn is_same_input(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}
