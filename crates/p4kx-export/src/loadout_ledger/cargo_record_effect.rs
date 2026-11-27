// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{LoadoutPathOperation, RecordLoadoutEffect, RecordPortSelection};
use p4kx_extraction::{CancelToken, FieldOrigin};

use super::Capture;
use super::path_proofs::PathProofs;
use super::vehicle_stage::PendingVehicle;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::AuthoredEntryPath;
use crate::{CatalogError, ExportAccountingError};

impl Capture<'_> {
    pub(super) fn observe_cargo_record_effect(
        &self,
        effect: &RecordLoadoutEffect<'_>,
    ) -> ControlFlow<()> {
        if self.stopped() {
            return self.fail(CatalogError::Cancelled);
        }
        match self.stage_cargo_record_effect(effect) {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }

    fn stage_cargo_record_effect(
        &self,
        effect: &RecordLoadoutEffect<'_>,
    ) -> Result<(), CatalogError> {
        let (owner, previous, winner, declared) = match effect {
            RecordLoadoutEffect::Override {
                owner_record_index,
                displaced,
                winner,
                ..
            } => (*owner_record_index, displaced, winner, false),
            RecordLoadoutEffect::SuppressDefault {
                owner_record_index,
                declared,
                winner,
            } => (*owner_record_index, declared, winner, true),
            _ => return Ok(()),
        };
        if !previous.name().eq_ignore_ascii_case(winner.name()) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let mut state = self.lock()?;
        if !state.proofs.contains(&owner) {
            return Err(ExportAccountingError::MissingFieldInspection.into());
        }
        let previous = selected_port(&state.paths, previous)?;
        let winner = selected_port(&state.paths, winner)?;
        if declared
            && let Some(previous) = &previous
            && previous.record_owner()? != owner
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let pending = state
            .vehicles
            .pending
            .as_mut()
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        pending
            .cargo_port_selectors
            .extend([previous, winner].into_iter().flatten());
        drop(state);
        Ok(())
    }
}

fn selected_port(
    paths: &PathProofs,
    operand: &RecordPortSelection<'_>,
) -> Result<Option<AuthoredEntryPath>, CatalogError> {
    let Some(path) = operand.name_proof() else {
        return Ok(None);
    };
    if path.value() != operand.name()
        || path
            .selection()
            .map(p4kx_assembly::LoadoutSelection::operation)
            != Some(LoadoutPathOperation::PortName)
    {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    Ok(Some(paths.get(path)?))
}

impl PendingVehicle {
    pub(super) fn cargo_membership(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<FieldOrigin, CatalogError> {
        let mut selectors = Vec::new();
        for proof in &self.cargo_port_selectors {
            checkpoint(cancel)?;
            proof.check_input(input)?;
            let (observed, _) = proof.selector_inputs()?;
            selectors.extend(observed);
        }
        // Port-name comparisons alone leave other cargo membership decisions unproven.
        Ok(input.origins().uses.origin(&selectors, false))
    }
}
