// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "loadout_ledger/files.rs"]
mod files;
#[path = "loadout_ledger/observer.rs"]
mod observer;
#[path = "loadout_ledger/operation.rs"]
mod operation;
#[path = "loadout_ledger/typed.rs"]
mod typed;

use std::collections::HashSet;
use std::ops::ControlFlow;
use std::sync::{Mutex, MutexGuard};

use p4kx_extraction::CancelToken;

use crate::CatalogError;
use crate::export_ledger::{ExportAccountingError, ExportLedger};

struct State {
    ledger: ExportLedger,
    seen: HashSet<u32>,
    proofs: HashSet<u32>,
    error: Option<CatalogError>,
}

impl State {
    fn new(source: &ExportLedger) -> Self {
        let mut state = Self {
            ledger: ExportLedger::default(),
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

/// Retains actual resolver observations until every worker has completed.
pub(crate) struct Capture {
    kind: crate::Kind,
    state: Mutex<State>,
    cancel: Option<CancelToken>,
}

impl Capture {
    pub(crate) fn new(source: &ExportLedger, cancel: Option<&CancelToken>) -> Self {
        Self::for_kind(source, cancel, crate::Kind::Loadouts)
    }

    pub(crate) fn for_kind(
        source: &ExportLedger,
        cancel: Option<&CancelToken>,
        kind: crate::Kind,
    ) -> Self {
        Self {
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
