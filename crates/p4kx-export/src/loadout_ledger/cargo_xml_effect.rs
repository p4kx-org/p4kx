// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{
    LoadoutFileObservation, LoadoutPath, LoadoutPathOperation, XmlPortReplacement,
};

use super::Capture;
use crate::catalog_query_session::checkpoint;
use crate::{CatalogError, ExportAccountingError};

impl Capture<'_> {
    pub(super) fn observe_cargo_xml_replacements(
        &self,
        root: &LoadoutFileObservation,
        caller: &LoadoutPath,
        replacements: &[XmlPortReplacement],
    ) -> ControlFlow<()> {
        match self.stage_cargo_xml_replacements(root, caller, replacements) {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }

    fn stage_cargo_xml_replacements(
        &self,
        root: &LoadoutFileObservation,
        caller: &LoadoutPath,
        replacements: &[XmlPortReplacement],
    ) -> Result<(), CatalogError> {
        checkpoint(self.cancel.as_ref())?;
        if replacements.is_empty() {
            return Ok(());
        }
        let request = caller
            .selection()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let mut state = self.lock()?;
        if !state.files.contains(root) {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        let mut selectors = Vec::new();
        for replacement in replacements {
            checkpoint(self.cancel.as_ref())?;
            if !state.files.contains(replacement.merge_document()) {
                return Err(ExportAccountingError::UnobservedFailure.into());
            }
            let displaced = replacement.displaced().name();
            let winner = replacement.winner().name();
            if !displaced.value().eq_ignore_ascii_case(winner.value())
                || [displaced, winner].into_iter().any(|operand| {
                    operand
                        .selection()
                        .map(p4kx_assembly::LoadoutSelection::operation)
                        != Some(LoadoutPathOperation::PortName)
                })
            {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            let displaced = state.paths.lookup(displaced)?;
            let winner = state.paths.lookup(winner)?;
            selectors.extend(state.graph.xml_replacement_selectors(
                root,
                request,
                replacement,
                [displaced, winner],
            )?);
        }
        state
            .vehicles
            .pending
            .as_mut()
            .ok_or(ExportAccountingError::UnobservedFailure)?
            .cargo_port_selectors
            .extend(selectors);
        drop(state);
        Ok(())
    }
}
