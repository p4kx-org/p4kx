// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::{DataForgeError, Guid, Location, Record, RecordText, Reference};

/// A borrowed original target admission, independent from descriptive equality.
#[derive(Debug, Clone, Copy)]
pub struct ReferenceOrigin<'borrow, 'source> {
    reference: &'borrow Reference<'source>,
}

impl<'borrow, 'source> ReferenceOrigin<'borrow, 'source> {
    pub(crate) const fn new(reference: &'borrow Reference<'source>) -> Self {
        Self { reference }
    }

    /// Returns whether the admitted target and this Record share one original DataForge owner.
    #[must_use]
    pub fn shares_source_with(self, record: &Record<'_>) -> bool {
        self.reference.owner.is_from(record)
    }

    /// Returns the original target Record position.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.reference.record_index
    }

    /// Returns the GUID of the original admitted Record, including an instance's owner.
    #[must_use]
    pub const fn record_guid(&self) -> Guid {
        self.reference.guid
    }

    /// Borrows the decoder-selected original Record without accepting another selection key.
    ///
    /// # Errors
    /// Returns the original missing-row error if its selected Record cannot be read.
    pub fn record(&self) -> Result<Record<'source>, DataForgeError> {
        self.reference.owner.record(self.reference.record_index)
    }

    /// Reads the exact original selected Record's name declaration without a new target selection.
    ///
    /// # Errors
    /// Returns the original selected header's invalid-string or missing-row error.
    pub fn record_name(&self) -> Result<RecordText<'source>, DataForgeError> {
        self.record()?.stored_name()
    }

    /// Returns the original admitted Record's normalized path.
    #[must_use]
    pub const fn record_path(&self) -> &'source str {
        self.reference.record_path
    }

    /// Returns the admitted Record type, inapplicable for an owned-instance target.
    #[must_use]
    pub const fn record_type(&self) -> Option<&'source str> {
        if self.reference.location.is_none() {
            Some(self.reference.type_name)
        } else {
            None
        }
    }

    /// Borrows the original field and array ordinals, absent for a Record target.
    /// Field ordinals refer to the visited struct's canonical fields, including inherited fields.
    #[must_use]
    pub fn instance_positions(&self) -> Option<&[u64]> {
        self.reference
            .location()
            .and_then(Location::original_positions)
    }

    /// Retains the decoder's existing immutable instance location, absent for a Record target.
    /// This shared view does not grant original-source admission.
    #[must_use]
    pub fn shared_instance_location(&self) -> Option<Arc<Location<'source>>> {
        self.reference.location.as_ref().map(Arc::clone)
    }

    /// Borrows the actual original instance location, absent for a Record target.
    #[must_use]
    pub fn instance_location(&self) -> Option<&Location<'source>> {
        self.reference.location()
    }
}

#[cfg(test)]
#[path = "reference_origin_name_tests.rs"]
mod name_tests;
