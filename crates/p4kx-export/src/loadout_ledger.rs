// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "loadout_ledger/file_dependencies.rs"]
mod file_dependencies;
#[path = "loadout_ledger/file_proofs.rs"]
mod file_proofs;
#[path = "loadout_ledger/files.rs"]
mod files;
#[path = "loadout_ledger/observer.rs"]
mod observer;
#[path = "loadout_ledger/operation.rs"]
mod operation;
#[path = "loadout_ledger/path_capture.rs"]
mod path_capture;
#[path = "loadout_ledger/path_observer.rs"]
mod path_observer;
#[path = "loadout_ledger/path_proofs.rs"]
mod path_proofs;
#[path = "loadout_ledger/typed.rs"]
mod typed;

use std::collections::HashSet;
use std::ops::ControlFlow;
use std::sync::{Mutex, MutexGuard};

use p4kx_extraction::CancelToken;

use self::file_proofs::FileProofs;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::FileGraph;
use crate::export_ledger::{ExportAccountingError, ExportLedger};
use crate::record_observation::RecordObservation;

struct State {
    ledger: ExportLedger,
    files: FileProofs,
    paths: path_proofs::PathProofs,
    references: reference_proofs::ReferenceProofs,
    classes: class_proofs::ClassProofs,
    graph: FileGraph,
    seen: HashSet<u32>,
    proofs: HashSet<u32>,
    error: Option<CatalogError>,
}

impl State {
    fn new(source: &ExportLedger) -> Self {
        let mut state = Self {
            ledger: ExportLedger::default(),
            files: FileProofs::default(),
            paths: path_proofs::PathProofs::default(),
            references: reference_proofs::ReferenceProofs::default(),
            classes: class_proofs::ClassProofs::default(),
            graph: FileGraph::default(),
            seen: HashSet::new(),
            proofs: HashSet::new(),
            error: None,
        };
        state.seed(source);
        state
    }

    fn seed(&mut self, source: &ExportLedger) {
        for proof in &source.field_inspections {
            self.seen.insert(proof.record_index);
            self.proofs.insert(proof.record_index);
        }
    }
}

/// Retains resolver proofs for cache lifetime and drains each caller's ledger delta.
pub(crate) struct Capture<'f> {
    input: Option<&'f OriginalCatalogInput<'f, 'f>>,
    kind: crate::Kind,
    state: Mutex<State>,
    cancel: Option<CancelToken>,
}

impl<'f> Capture<'f> {
    #[cfg(test)]
    pub(crate) fn new(source: &ExportLedger, cancel: Option<&CancelToken>) -> Self {
        Self::create(source, cancel, crate::Kind::Loadouts)
    }

    pub(crate) fn for_input(
        input: &'f OriginalCatalogInput<'f, 'f>,
        source: &ExportLedger,
        cancel: Option<&CancelToken>,
        kind: crate::Kind,
    ) -> Self {
        let mut capture = Self::create(source, cancel, kind);
        capture.input = Some(input);
        capture
    }

    pub(crate) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), CatalogError> {
        if self
            .input
            .is_none_or(|original| !std::ptr::eq(original, input))
        {
            return Err(p4kx_extraction::SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }

    fn admission<'read, 'source>(
        &self,
        record: &'read p4kx_extraction::Record<'source>,
    ) -> Result<RecordObservation<'read, 'source>, CatalogError> {
        self.input.map_or_else(
            || Ok(RecordObservation::accounting(record)),
            |input| RecordObservation::capture(input, record),
        )
    }

    fn check_record(&self, record: &p4kx_extraction::Record<'_>) -> Result<(), CatalogError> {
        if let Some(input) = self.input {
            input.record_context(record)?;
        }
        Ok(())
    }

    fn create(source: &ExportLedger, cancel: Option<&CancelToken>, kind: crate::Kind) -> Self {
        Self {
            input: None,
            kind,
            state: Mutex::new(State::new(source)),
            cancel: cancel.cloned(),
        }
    }

    pub(crate) fn seed_proofs(&self, source: &ExportLedger) -> Result<(), CatalogError> {
        self.lock()?.seed(source);
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>, ExportAccountingError> {
        crate::export_evidence_lock::lock(&self.state)
    }

    fn record_error(&self, error: CatalogError) {
        if let Ok(mut state) = self.lock() {
            state.error.get_or_insert(error);
        }
    }

    fn fail(&self, error: CatalogError) -> ControlFlow<()> {
        self.record_error(error);
        ControlFlow::Break(())
    }

    fn stopped(&self) -> bool {
        self.cancel.as_ref().is_some_and(CancelToken::is_cancelled)
            || self.lock().map_or(true, |state| state.error.is_some())
    }

    pub(crate) fn missing_root(&self) {
        self.record_error(ExportAccountingError::MissingFieldInspection.into());
    }

    /// Propagates observer, poisoning or cancellation failures before publication.
    pub(crate) fn finish(&self) -> Result<ExportLedger, CatalogError> {
        let mut state = self.lock()?;
        if let Some(error) = state.error.take() {
            return Err(error);
        }
        if self.cancel.as_ref().is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        Ok(std::mem::take(&mut state.ledger))
    }
}

#[cfg(test)]
#[path = "loadout_ledger/tests.rs"]
mod tests;

#[path = "loadout_ledger/request_examples.rs"]
mod request_examples;

#[path = "loadout_ledger/reference_capture.rs"]
mod reference_capture;
#[path = "loadout_ledger/reference_proofs.rs"]
mod reference_proofs;

#[path = "loadout_ledger/class_capture.rs"]
mod class_capture;
#[path = "loadout_ledger/class_proofs.rs"]
mod class_proofs;
