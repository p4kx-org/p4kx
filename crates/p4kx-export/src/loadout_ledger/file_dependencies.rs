// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{
    LoadoutDocumentKind, LoadoutError, LoadoutFileObservation, LoadoutPath, LoadoutRequest,
};

use crate::{CatalogError, ExportAccountingError};

use super::Capture;

impl Capture<'_> {
    fn dependency_parent(&self, original: &LoadoutFileObservation) -> Result<(), CatalogError> {
        if original.kind() != LoadoutDocumentKind::XmlLoadout
            || original.read_outcome().is_err()
            || !matches!(original.projection_outcome(), Some(Ok(())))
            || !self.lock()?.files.contains(original)
        {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        Ok(())
    }

    pub(super) fn file_dependency(
        &self,
        parent: &LoadoutFileObservation,
        original: &LoadoutFileObservation,
        request: &LoadoutPath,
        dependency: &LoadoutRequest,
    ) -> ControlFlow<()> {
        let result = (|| {
            self.dependency_parent(parent)?;
            if original.kind() != LoadoutDocumentKind::XmlLoadout
                || !self.lock()?.files.contains(original)
                || (original.read_outcome().is_ok() && original.projection_outcome().is_none())
            {
                return Err(ExportAccountingError::UnobservedFailure.into());
            }
            #[cfg(test)]
            if self.input.is_none() {
                return Ok(());
            }
            let state = self.lock()?;
            let candidate = state.paths.get(request)?;
            let (token, selection) = state.paths.dependency(parent, dependency)?;
            state.graph.check_candidate(original, &candidate)?;
            state.graph.dependency(parent, original, token, selection)
        })();
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }

    pub(super) fn file_dependency_refused(
        &self,
        parent: &LoadoutFileObservation,
        dependency: &LoadoutRequest,
        error: &LoadoutError,
    ) -> ControlFlow<()> {
        if matches!(error, LoadoutError::ObservationStopped) {
            return self.fail(CatalogError::Cancelled);
        }
        let result = (|| {
            self.dependency_parent(parent)?;
            #[cfg(test)]
            if self.input.is_none() {
                return Ok(());
            }
            let state = self.lock()?;
            let (token, _) = state.paths.dependency(parent, dependency)?;
            state.graph.refused(parent, token)
        })();
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }
}
