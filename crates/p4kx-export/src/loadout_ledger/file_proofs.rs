// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use p4kx_assembly::{LoadoutFileObservation, LoadoutFileOperationKey};

use crate::ExportAccountingError;

struct Proof {
    original: LoadoutFileObservation,
    projection_observed: bool,
}

/// Exact original operations and separately observed phases retained for cache lifetime.
#[derive(Default)]
pub(super) struct FileProofs {
    operations: HashMap<LoadoutFileOperationKey, Proof>,
}

impl FileProofs {
    pub(super) fn read(
        &mut self,
        original: &LoadoutFileObservation,
    ) -> Result<(), ExportAccountingError> {
        match self.operations.entry(original.operation_key()) {
            Entry::Occupied(_) => Err(ExportAccountingError::UnobservedFailure),
            Entry::Vacant(entry) => {
                entry.insert(Proof {
                    original: original.clone(),
                    projection_observed: false,
                });
                Ok(())
            }
        }
    }

    pub(super) fn projected(
        &mut self,
        original: &LoadoutFileObservation,
    ) -> Result<(), ExportAccountingError> {
        let proof = self
            .operations
            .get_mut(&original.operation_key())
            .filter(|proof| proof.original.is_same_operation(original))
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        if original.read_outcome().is_err()
            || original.projection_outcome().is_none()
            || proof.projection_observed
        {
            return Err(ExportAccountingError::UnobservedFailure);
        }
        proof.projection_observed = true;
        Ok(())
    }

    pub(super) fn contains(&self, original: &LoadoutFileObservation) -> bool {
        self.operations
            .get(&original.operation_key())
            .is_some_and(|proof| {
                proof.original.is_same_operation(original)
                    && proof.projection_observed == original.projection_outcome().is_some()
            })
    }
}

#[cfg(test)]
#[path = "file_proofs_tests.rs"]
mod tests;
