// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Guid, Location, Record, Reference};

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

    /// Borrows the actual original instance location, absent for a Record target.
    #[must_use]
    pub fn instance_location(&self) -> Option<&Location<'source>> {
        self.reference.location()
    }
}
