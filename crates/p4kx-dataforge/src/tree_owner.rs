// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use crate::{DataForge, Record};

/// Borrowed identity of the DataForge that supplied a canonical tree.
#[derive(Clone, Copy)]
pub(crate) struct TreeOwner<'source> {
    forge: &'source DataForge<'source>,
}

impl<'source> TreeOwner<'source> {
    /// Retains the actual canonical reader's borrowed owner.
    pub(crate) const fn new(forge: &'source DataForge<'source>) -> Self {
        Self { forge }
    }

    /// Compares the original borrowed owner without using descriptive identities.
    pub(crate) fn is_from(self, record: &Record<'_>) -> bool {
        record.is_from(self.forge)
    }
}

impl fmt::Debug for TreeOwner<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TreeOwner").finish_non_exhaustive()
    }
}
