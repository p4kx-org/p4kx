// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::layout::Layout;
use crate::{DataForge, DataForgeError};

/// The actual DataForge whose immutable layout index is shared by one statistics operation.
#[derive(Debug)]
pub(crate) struct SharedLayouts<'source> {
    forge: &'source DataForge<'source>,
}

impl<'source> SharedLayouts<'source> {
    pub(crate) const fn new(forge: &'source DataForge<'source>) -> Self {
        Self { forge }
    }

    pub(crate) fn get(
        &self,
        forge: &'source DataForge<'source>,
        struct_index: u32,
    ) -> Result<Layout<'source>, DataForgeError> {
        if !std::ptr::eq(self.forge, forge) {
            return Err(DataForgeError::LayoutInvariant);
        }
        Layout::from_index(forge, forge.source_layouts()?, struct_index)
    }
}
