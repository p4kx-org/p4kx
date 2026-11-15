// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use super::{HircKind, HircValue};
use crate::BankError;
use crate::bank_structure::{OBJECT_HEADER_LEN, OBJECT_ID_LEN};

/// One object of the `HIRC` section, with its ID and its decoded value.
#[derive(Debug, Clone, PartialEq)]
pub struct HircObject {
    /// The type of the object.
    pub kind: HircKind,
    /// The object ID.
    pub id: u32,
    /// What this reader decodes from the body.
    pub value: HircValue,
    pub(crate) body: Range<usize>,
}

impl HircObject {
    /// Returns the whole object span in the decoded bank, including its envelope and ID.
    #[must_use]
    pub const fn byte_range(&self) -> Range<usize> {
        self.body.start - OBJECT_HEADER_LEN..self.body.end
    }

    /// Returns the original object type byte in the decoded bank.
    #[must_use]
    pub const fn kind_byte_range(&self) -> Range<usize> {
        let start = self.body.start - OBJECT_HEADER_LEN;
        start..start + 1
    }

    /// Returns the four bytes of the object ID in the decoded bank.
    #[must_use]
    pub const fn id_byte_range(&self) -> Range<usize> {
        self.body.start..self.body.start + OBJECT_ID_LEN
    }

    /// Returns why the body could not be decoded, for a [`HircValue::Malformed`] object.
    #[must_use]
    pub const fn error(&self) -> Option<&BankError> {
        match &self.value {
            HircValue::Malformed(error) => Some(error),
            _ => None,
        }
    }
}
