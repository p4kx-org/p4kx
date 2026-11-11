// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Mutex;

use p4kx_extraction::{CancelToken, CanonicalReadObserver, DataForgeError, Record, RecordTree};

use crate::build::original_input::OriginalCatalogInput;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_evidence_lock;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::record_observation::RecordObservation;
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
pub(crate) struct BuilderCapture<'f> {
    input: Option<&'f OriginalCatalogInput<'f, 'f>>,
    context: Mutex<Context>,
    state: Mutex<State>,
    cancel: Option<CancelToken>,
}

impl<'f> BuilderCapture<'f> {
    #[cfg(test)]
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

    #[cfg(test)]
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

    pub(crate) fn for_input_root(
        input: &'f OriginalCatalogInput<'f, 'f>,
        record: &Record<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        input.record_context(record)?;
        Ok(Self::new(
            Context {
                active: false,
                kind: None,
                primary: Some(record.index()),
            },
            cancel,
        )
        .with_input(input))
    }

    pub(crate) fn for_input_kind(
        input: &'f OriginalCatalogInput<'f, 'f>,
        kind: Kind,
        cancel: Option<&CancelToken>,
    ) -> Self {
        Self::new(
            Context {
                active: true,
                kind: Some(kind),
                primary: None,
            },
            cancel,
        )
        .with_input(input)
    }

    const fn with_input(mut self, input: &'f OriginalCatalogInput<'f, 'f>) -> Self {
        self.input = Some(input);
        self
    }

    fn new(context: Context, cancel: Option<&CancelToken>) -> Self {
        Self {
            input: None,
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
        let admission = match self.input {
            Some(input) => RecordObservation::capture(input, record)?,
            None => RecordObservation::accounting(record),
        };
        let mut ledger = ExportLedger::default();
        let disposition = match outcome {
            Ok(tree) => {
                ledger.merge(crate::export_field_walk::inspect_observed(
                    self.input,
                    tree,
                    &admission,
                    context.kind,
                    ExportSourceRole::Dependency,
                    self.cancel.as_ref(),
                )?);
                SourceDisposition::Projected
            }
            Err(_) => SourceDisposition::ReadFailed,
        };
        let mut observation =
            admission.source(context.kind, ExportSourceRole::Dependency, disposition)?;
        if let Err(error) = outcome {
            observation = observation.diagnosed(crate::export_source_failure::diagnosis(error));
        }
        ledger.observe(observation);
        Ok(ledger)
    }
}

impl CanonicalReadObserver for BuilderCapture<'_> {
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

#[cfg(test)]
#[path = "borrowed_original_observer_tests.rs"]
mod original_input_tests;
