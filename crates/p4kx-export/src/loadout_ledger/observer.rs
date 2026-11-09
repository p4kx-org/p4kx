// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{LoadoutReadEvent, LoadoutReadObserver};
use p4kx_extraction::{DataForgeError, Record, RecordTree};

use crate::CatalogError;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportSourceRole,
};
use crate::export_ledger::{ExportAccountingError, SourceDisposition, SourceObservation};

use super::Capture;

impl Capture {
    fn canonical(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        let mut ledger = match outcome {
            Ok(tree) => match crate::export_field_walk::inspect(
                tree,
                record,
                Some(self.kind),
                ExportSourceRole::Dependency,
                self.cancel.as_ref(),
            ) {
                Ok(ledger) => ledger,
                Err(error) => return self.fail(error),
            },
            Err(_) => crate::export_ledger::ExportLedger::default(),
        };
        let disposition = if outcome.is_ok() {
            SourceDisposition::Projected
        } else {
            SourceDisposition::ReadFailed
        };
        let mut source =
            SourceObservation::new(self.kind, record, ExportSourceRole::Dependency, disposition);
        if let Err(error) = outcome {
            source = source.diagnosed(crate::export_source_failure::diagnosis(error));
        }
        ledger.observe(source);
        if outcome.is_err() {
            ledger.incomplete_inventory(self.kind);
        }
        let Ok(mut state) = self.lock() else {
            return ControlFlow::Break(());
        };
        state.seen.insert(record.index());
        if outcome.is_ok() {
            state.proofs.insert(record.index());
        }
        state.ledger.merge(ledger);
        drop(state);
        ControlFlow::Continue(())
    }

    fn reuse(&self, record: &Record<'_>, needs_proof: bool) -> ControlFlow<()> {
        let Ok(state) = self.lock() else {
            return ControlFlow::Break(());
        };
        let available = if needs_proof {
            state.proofs.contains(&record.index())
        } else {
            state.seen.contains(&record.index())
        };
        drop(state);
        if available {
            ControlFlow::Continue(())
        } else {
            self.fail(ExportAccountingError::MissingFieldInspection.into())
        }
    }

    fn rejected(&self, record: &Record<'_>, code: &'static str) -> ControlFlow<()> {
        let unreadable = record.type_name().is_err();
        let (role, disposition) = if unreadable {
            (
                ExportSourceRole::SelectionCandidate,
                SourceDisposition::ReadFailed,
            )
        } else {
            (
                ExportSourceRole::Dependency,
                SourceDisposition::SkippedBeforeRead,
            )
        };
        let Ok(mut state) = self.lock() else {
            return ControlFlow::Break(());
        };
        state.seen.insert(record.index());
        state
            .ledger
            .observe(SourceObservation::new(self.kind, record, role, disposition));
        let mut diagnostic = super::operation::diagnostic(
            record,
            ExportDiagnosticStage::Selection,
            ExportDiagnosticCode::UnsupportedValue,
            ExportDiagnosticSeverity::Error,
            code,
        );
        diagnostic.kind = Some(self.kind);
        diagnostic.source_role = Some(role);
        state.ledger.field_diagnostic(diagnostic);
        drop(state);
        ControlFlow::Continue(())
    }
}

impl LoadoutReadObserver for Capture {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        if self.stopped() {
            return self.fail(CatalogError::Cancelled);
        }
        match event {
            LoadoutReadEvent::FileRead {
                source,
                kind,
                outcome,
            } => self.file_read(source, kind, outcome),
            LoadoutReadEvent::FileProjection {
                source,
                kind,
                outcome,
            } => self.file_projection(source, kind, outcome),
            LoadoutReadEvent::FileReused { observation } => self.file_reused(observation),
            LoadoutReadEvent::InventoryIncomplete => self.incomplete_files(),
            LoadoutReadEvent::CanonicalRead { record, outcome } => self.canonical(record, outcome),
            LoadoutReadEvent::ProvidedTree { record, .. } => self.reuse(record, true),
            LoadoutReadEvent::Reused { record } => self.reuse(record, false),
            LoadoutReadEvent::Rejected { record, error } => self.rejected(record, error.kind()),
        }
    }
}

impl p4kx_extraction::CanonicalReadObserver for Capture {
    fn observe(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        if self.stopped() {
            return self.fail(CatalogError::Cancelled);
        }
        self.canonical(record, outcome)
    }
}
