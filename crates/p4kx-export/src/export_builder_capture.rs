// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Mutex;

use p4kx_extraction::{CancelToken, CanonicalReadObserver, DataForgeError, Record, RecordTree};

use crate::export_diagnostic::ExportSourceRole;
use crate::export_evidence_lock;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::{CatalogError, Kind};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Context {
    active: bool,
    kind: Option<Kind>,
    primary: Option<u32>,
}

#[derive(Default)]
struct State {
    ledger: ExportLedger,
    fatal: Option<CatalogError>,
}

/// Observes actual secondary canonical reads while preserving the caller's original fatal error.
pub(crate) struct BuilderCapture {
    context: Mutex<Context>,
    state: Mutex<State>,
    cancel: Option<CancelToken>,
}

impl BuilderCapture {
    pub(crate) fn for_root(index: u32, cancel: Option<&CancelToken>) -> Self {
        Self::new(
            Context {
                active: false,
                kind: None,
                primary: Some(index),
            },
            cancel,
        )
    }

    pub(crate) fn for_kind(kind: Kind, cancel: Option<&CancelToken>) -> Self {
        Self::new(
            Context {
                active: true,
                kind: Some(kind),
                primary: None,
            },
            cancel,
        )
    }

    fn new(context: Context, cancel: Option<&CancelToken>) -> Self {
        Self {
            context: Mutex::new(context),
            state: Mutex::new(State::default()),
            cancel: cancel.cloned(),
        }
    }

    pub(crate) fn activate(&self, kind: Kind) -> Result<(), CatalogError> {
        let mut context = export_evidence_lock::lock(&self.context)?;
        context.kind = Some(kind);
        context.active = true;
        drop(context);
        Ok(())
    }

    /// Returns the original fatal error before mapped rows or compatibility failures are accepted.
    pub(crate) fn finish(&self) -> Result<ExportLedger, CatalogError> {
        let mut state = export_evidence_lock::lock(&self.state)?;
        if let Some(error) = state.fatal.take() {
            return Err(error);
        }
        drop(export_evidence_lock::lock(&self.context)?);
        Ok(std::mem::take(&mut state.ledger))
    }

    fn record(
        &self,
        context: Context,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> Result<ExportLedger, CatalogError> {
        let mut ledger = ExportLedger::default();
        let disposition = match outcome {
            Ok(tree) => {
                ledger.merge(crate::export_field_walk::inspect(
                    tree,
                    record,
                    context.kind,
                    ExportSourceRole::Dependency,
                    self.cancel.as_ref(),
                )?);
                SourceDisposition::Projected
            }
            Err(_) => SourceDisposition::ReadFailed,
        };
        let mut observation = context.kind.map_or_else(
            || SourceObservation::catalog_source(record, ExportSourceRole::Dependency, disposition),
            |kind| SourceObservation::new(kind, record, ExportSourceRole::Dependency, disposition),
        );
        if let Err(error) = outcome {
            observation = observation.diagnosed(crate::export_source_failure::diagnosis(error));
        }
        ledger.observe(observation);
        Ok(ledger)
    }
}

impl CanonicalReadObserver for BuilderCapture {
    fn observe(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        let Ok(context) = export_evidence_lock::lock(&self.context).map(|context| *context) else {
            return ControlFlow::Break(());
        };
        if !context.active || context.primary == Some(record.index()) {
            return ControlFlow::Continue(());
        }
        let Ok(mut state) = export_evidence_lock::lock(&self.state) else {
            return ControlFlow::Break(());
        };
        if state.fatal.is_some() {
            return ControlFlow::Break(());
        }
        match self.record(context, record, outcome) {
            Ok(ledger) => {
                state.ledger.merge(ledger);
                drop(state);
                ControlFlow::Continue(())
            }
            Err(error) => {
                state.fatal = Some(error);
                drop(state);
                ControlFlow::Break(())
            }
        }
    }
}
