// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use crate::dataforge;
use crate::rows::{NO_PARENT, StructDef};
use crate::{DataForge, DataForgeError};

/// Returns the original struct and all ancestors, nearest first, rejecting repeated indices.
pub(crate) fn chain(
    forge: &DataForge<'_>,
    struct_index: u32,
) -> Result<Vec<(u32, StructDef)>, DataForgeError> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut current = struct_index;
    loop {
        if !seen.insert(current) {
            return Err(DataForgeError::StructCycle { struct_index });
        }
        let row = usize::try_from(current)
            .ok()
            .and_then(|index| forge.structs.get(index))
            .ok_or_else(|| {
                dataforge::out_of_range("struct", struct_index as usize, "struct", current.into())
            })?;
        chain.push((current, row));
        if row.parent == NO_PARENT {
            return Ok(chain);
        }
        current = row.parent;
    }
}
