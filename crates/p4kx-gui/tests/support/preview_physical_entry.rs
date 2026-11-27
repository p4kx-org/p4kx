// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gui::EntryIndex;

pub(crate) fn browser_id(index: &EntryIndex, physical_position: u32) -> Option<u32> {
    let position = physical_position as usize;
    index
        .tree()
        .folder(index.root())
        .entries()
        .find(|&id| index.tree().position(id) == Some(position))
}
