// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_assembly::LoadoutFileObservation;

use super::{Operation, State};
use crate::ExportAccountingError;
use crate::catalog_read::original::AuthoredEntryPath;

impl State {
    pub(super) fn operation(&self, index: usize) -> Result<&Operation, ExportAccountingError> {
        self.operations
            .get(index)
            .ok_or(ExportAccountingError::UnobservedFailure)
    }

    pub(super) fn operation_mut(
        &mut self,
        index: usize,
    ) -> Result<&mut Operation, ExportAccountingError> {
        self.operations
            .get_mut(index)
            .ok_or(ExportAccountingError::UnobservedFailure)
    }

    pub(super) const fn open(&self) -> Result<(), ExportAccountingError> {
        if self.sealed {
            return Err(ExportAccountingError::HealthCaptureClosed);
        }
        Ok(())
    }

    pub(super) fn initial_selectors(
        &self,
        original: usize,
    ) -> Result<Vec<&AuthoredEntryPath>, ExportAccountingError> {
        let mut seen = HashSet::new();
        let mut next = Some(original);
        let mut selected = Vec::new();
        while let Some(index) = next {
            if !seen.insert(index) {
                return Err(ExportAccountingError::ConflictingSourceDatum);
            }
            let operation = self.operation(index)?;
            let first = &operation.first_caller;
            if index != original {
                selected.push(&operation.candidate);
            }
            selected.extend(
                first
                    .iter()
                    .filter(|path| !path.is_same_recipe(&operation.candidate)),
            );
            next = operation.first_parent;
        }
        Ok(selected)
    }

    pub(super) fn position(&self, original: &LoadoutFileObservation) -> Option<usize> {
        let index = *self.positions.get(&original.operation_key())?;
        self.operations
            .get(index)?
            .original
            .is_same_operation(original)
            .then_some(index)
    }
}
