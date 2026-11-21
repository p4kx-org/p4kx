// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

/// Selected scene inputs retained by one backend invocation.
#[derive(Default)]
pub(crate) struct RenderInputs {
    pub(crate) bitmaps: BTreeSet<u16>,
    pub(crate) items: Vec<usize>,
    pub(crate) masks: Vec<usize>,
}

impl RenderInputs {
    pub(crate) fn item(&mut self, ordinal: usize) {
        if self.items.last() != Some(&ordinal) {
            self.items.push(ordinal);
        }
    }
}
