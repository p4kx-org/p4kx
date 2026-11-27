// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::LoadoutReferenceSelection;
use p4kx_extraction::Guid;

use crate::catalog_read::original::{ExampleBodyReceipt, LoadoutTargetReceipt};
use crate::health_diagnostic_observation::ExampleObservation;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

use super::Capture;

impl Capture<'_> {
    pub(super) fn retain_reference(
        &self,
        selection: &LoadoutReferenceSelection,
        target: LoadoutTargetReceipt,
    ) -> Result<(), CatalogError> {
        self.lock()?.references.insert(selection, target)?;
        Ok(())
    }

    pub(super) fn reference_example(
        &self,
        admission: &RecordObservation<'_, '_>,
        selection: &LoadoutReferenceSelection,
        guid: Guid,
    ) -> Result<ExampleObservation, CatalogError> {
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let target = self.lock()?.references.get(selection, guid)?;
        let body = ExampleBodyReceipt::loadout_target(input, admission.subject().clone(), target)?;
        let (field, source_type, target_guid) = body.description()?;
        Ok(ExampleObservation::captured(
            admission.subject().clone(),
            field.as_deref(),
            source_type.as_deref(),
            target_guid,
            body,
        )?)
    }
}
