// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue};

use super::{EntryCapture, EntryMetadata, EntryReceipt, OriginalDatum, PhysicalEntryRef};
use crate::CatalogError;

impl EntryReceipt {
    pub(crate) fn physical_reference(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<OriginalDatum<PhysicalEntryRef>, CatalogError> {
        self.check_capture(capture)?;
        Self::reference_of(self.outer_directory()?)
    }

    pub(crate) fn payload_reference(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<OriginalDatum<PhysicalEntryRef>, CatalogError> {
        self.check_capture(capture)?;
        Self::reference_of(self.payload_entry()?)
    }

    fn reference_of(
        origin: &EntryMetadata,
    ) -> Result<OriginalDatum<PhysicalEntryRef>, CatalogError> {
        let value = PhysicalEntryRef {
            input_ref: origin.source_ref().clone(),
            entry_index: origin.index(),
        };
        let uses = vec![
            origin.metadata("path", SourceUseRole::Selector)?,
            origin.metadata("entryIndex", SourceUseRole::Selector)?,
        ];
        Ok(OriginalDatum::observed(
            SourceValue::Present(value),
            uses,
            true,
        ))
    }
}
