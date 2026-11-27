// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{LoadoutFailurePath, LoadoutFileError};

use crate::catalog_read::original::ExampleBodyReceipt;
use crate::health_diagnostic_observation::ExampleObservation;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, HealthText};

use super::Capture;

impl Capture<'_> {
    pub(super) fn request_example(
        &self,
        admission: &RecordObservation<'_, '_>,
        request: &LoadoutFailurePath,
    ) -> Result<ExampleObservation, CatalogError> {
        let Some(input) = self.input else {
            let mut example = super::operation::example(None, admission)?;
            example.field = Some(HealthText::new(request.value()));
            return Ok(example);
        };
        let caller = request
            .caller()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let (path, selectors) = {
            let state = self.lock()?;
            if request.attempts().is_empty()
                || !request.attempts().iter().all(|attempt| {
                    state.files.contains(attempt)
                        && matches!(attempt.read_outcome(), Err(LoadoutFileError::NotFound))
                })
            {
                return Err(ExportAccountingError::UnobservedFailure.into());
            }
            let path = state.paths.get(request.path())?;
            let mut selectors = state.paths.request(caller)?;
            for include in request.includes() {
                selectors.push(state.paths.get(include)?);
            }
            selectors.retain(|selector| !selector.is_same_recipe(&path));
            drop(state);
            (path, selectors)
        };
        let body = ExampleBodyReceipt::requested_loadout(
            input,
            admission.subject().clone(),
            path,
            selectors,
        )?;
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
