// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::DataForgeError;

/// Reserves metadata elements without treating allocation refusal as invalid input.
pub(crate) fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    part: &'static str,
) -> Result<(), DataForgeError> {
    let elements = values
        .len()
        .checked_add(additional)
        .ok_or(DataForgeError::TooLarge { part })?;
    values
        .try_reserve(additional)
        .map_err(|_| DataForgeError::LayoutAllocationRefused {
            part,
            elements: elements as u64,
        })
}
