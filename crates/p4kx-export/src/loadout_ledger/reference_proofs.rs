// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use p4kx_assembly::LoadoutReferenceSelection;
use p4kx_extraction::Guid;

use crate::ExportAccountingError;
use crate::catalog_read::original::LoadoutTargetReceipt;

/// Deferred actual Reference data keyed only by the original accepted SDK getter.
#[derive(Default)]
pub(super) struct ReferenceProofs {
    references: HashMap<LoadoutReferenceSelection, LoadoutTargetReceipt>,
}

impl ReferenceProofs {
    pub(super) fn insert(
        &mut self,
        selection: &LoadoutReferenceSelection,
        target: LoadoutTargetReceipt,
    ) -> Result<(), ExportAccountingError> {
        match self.references.entry(selection.clone()) {
            Entry::Vacant(slot) => {
                slot.insert(target);
                Ok(())
            }
            Entry::Occupied(_) => Err(ExportAccountingError::ConflictingSourceDatum),
        }
    }

    pub(super) fn get(
        &self,
        selection: &LoadoutReferenceSelection,
        expected: Guid,
    ) -> Result<LoadoutTargetReceipt, ExportAccountingError> {
        let target = self
            .references
            .get(selection)
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        if target.value()? != expected {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(target.clone())
    }
}
