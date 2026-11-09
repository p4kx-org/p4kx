// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::DataType;
use crate::layout::FieldSlot;

/// The original storage declaration used to decode one field and its array elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldStorage {
    data_type: DataType,
    array: bool,
    target: u16,
}

impl FieldStorage {
    pub(crate) const fn from_slot(slot: &FieldSlot<'_>) -> Self {
        Self {
            data_type: slot.data_type,
            array: slot.array,
            target: slot.target,
        }
    }

    /// Returns the actual declared element data type.
    #[must_use]
    pub const fn data_type(&self) -> DataType {
        self.data_type
    }

    /// Returns whether the original field stores an array.
    #[must_use]
    pub const fn is_array(&self) -> bool {
        self.array
    }

    /// Returns the original declared target word independently of a resolved value.
    #[must_use]
    pub const fn target(&self) -> u16 {
        self.target
    }
}
