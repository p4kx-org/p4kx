// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use p4kx_assembly::LoadoutClassSelection;
use p4kx_extraction::{Guid, Record};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{LoadoutClassNameReceipt, LoadoutTargetReceipt};
use crate::{CatalogError, ExportAccountingError};

struct ClassProof {
    getter: LoadoutClassNameReceipt,
    selected: Option<LoadoutTargetReceipt>,
}

/// Actual getter operations joined only to their observed selected Record borrows.
#[derive(Default)]
pub(super) struct ClassProofs {
    classes: HashMap<LoadoutClassSelection, ClassProof>,
}

impl ClassProofs {
    pub(super) fn getter(
        &mut self,
        selection: &LoadoutClassSelection,
        getter: LoadoutClassNameReceipt,
    ) -> Result<(), ExportAccountingError> {
        match self.classes.entry(selection.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(ClassProof {
                    getter,
                    selected: None,
                });
                Ok(())
            }
            Entry::Occupied(_) => Err(ExportAccountingError::ConflictingSourceDatum),
        }
    }

    pub(super) fn selected(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        selection: &LoadoutClassSelection,
        record: &Record<'_>,
    ) -> Result<(), CatalogError> {
        let proof = self
            .classes
            .get_mut(selection)
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        if let Some(original) = &proof.selected {
            return original.repeat_class(input, record);
        }
        let selected = LoadoutTargetReceipt::selected_class(input, proof.getter.clone(), record)?;
        proof.selected = Some(selected);
        Ok(())
    }

    pub(super) fn get(
        &self,
        selection: &LoadoutClassSelection,
        expected: Guid,
    ) -> Result<LoadoutTargetReceipt, ExportAccountingError> {
        let proof = self
            .classes
            .get(selection)
            .and_then(|proof| proof.selected.as_ref())
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        if proof.value()? != expected {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(proof.clone())
    }
}
