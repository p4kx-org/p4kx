// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{
    LoadoutClassSelection, LoadoutPath, LoadoutPathObserver, LoadoutReadEvent, LoadoutReadObserver,
    LoadoutSelection, XmlLoadoutClassSite, XmlLoadoutPathSite,
};
use p4kx_extraction::{DataForgeError, Record, RecordTree};

use crate::CatalogError;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportSourceRole,
};
use crate::export_ledger::{ExportAccountingError, SourceDisposition};

use super::Capture;

impl Capture<'_> {
    fn canonical(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        let admission = match self.admission(record) {
            Ok(admission) => admission,
            Err(error) => return self.fail(error),
        };
        let mut ledger = match outcome {
            Ok(tree) => match crate::export_field_walk::inspect_observed(
                self.input,
                tree,
                &admission,
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
            match admission.source(Some(self.kind), ExportSourceRole::Dependency, disposition) {
                Ok(source) => source,
                Err(error) => return self.fail(error),
            };
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
        if let Err(error) = self.check_record(record) {
            return self.fail(error);
        }
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
        if let Err(error) = self.check_record(record) {
            return self.fail(error);
        }
        let admission = match self.admission(record) {
            Ok(admission) => admission,
            Err(error) => return self.fail(error),
        };
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
        let source = match admission.source(Some(self.kind), role, disposition) {
            Ok(source) => source,
            Err(error) => {
                state.error.get_or_insert(error);
                return ControlFlow::Break(());
            }
        };
        state.ledger.observe(source);
        let diagnostic = super::operation::diagnostic(
            self.input,
            &admission,
            ExportDiagnosticStage::Selection,
            ExportDiagnosticCode::UnsupportedValue,
            ExportDiagnosticSeverity::Error,
            code,
        );
        let mut diagnostic = match diagnostic {
            Ok(diagnostic) => diagnostic,
            Err(error) => {
                state.error.get_or_insert(error);
                return ControlFlow::Break(());
            }
        };
        diagnostic.kind = Some(self.kind);
        diagnostic.source_role = Some(role);
        state.ledger.field_diagnostic(diagnostic);
        drop(state);
        ControlFlow::Continue(())
    }
}

impl LoadoutReadObserver for Capture<'_> {
    fn with_record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        self.capture_record_paths(record, tree, parse)
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        self.capture_xml_path(selection, site)
    }

    fn xml_class(
        &self,
        selection: &LoadoutClassSelection,
        site: &XmlLoadoutClassSite<'_>,
    ) -> ControlFlow<()> {
        self.class_xml(selection, site)
    }

    fn class_record(
        &self,
        selection: &LoadoutClassSelection,
        record: &Record<'_>,
    ) -> ControlFlow<()> {
        self.class_selected(selection, record)
    }

    fn derived_path(
        &self,
        selection: &LoadoutSelection,
        inputs: &[&LoadoutPath],
        value: &str,
    ) -> ControlFlow<()> {
        self.capture_derived_path(selection, inputs, value)
    }

    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        if self.stopped() {
            return self.fail(CatalogError::Cancelled);
        }
        match event {
            LoadoutReadEvent::FileRead {
                observation,
                request,
                caller,
            } => self.file_read(observation, request, caller),
            LoadoutReadEvent::FileProjection {
                observation,
                request,
                caller,
            } => self.file_projection(observation, request, caller),
            LoadoutReadEvent::FileReused {
                observation,
                request,
                caller,
            } => self.file_reused(observation, request, caller),
            LoadoutReadEvent::FileDependency {
                parent,
                observation,
                request,
                dependency,
            } => self.file_dependency(parent, observation, request, dependency),
            LoadoutReadEvent::FileDependencyRefused {
                parent,
                dependency,
                error,
            } => self.file_dependency_refused(parent, dependency, error),
            LoadoutReadEvent::InventoryIncomplete => self.incomplete_files(),
            LoadoutReadEvent::CanonicalRead { record, outcome } => self.canonical(record, outcome),
            LoadoutReadEvent::ProvidedTree { record, tree } => {
                if !tree.is_from(record) {
                    return self.fail(p4kx_extraction::SourceEvidenceError::ForeignEvidence.into());
                }
                self.reuse(record, true)
            }
            LoadoutReadEvent::Reused { record } => self.reuse(record, false),
            LoadoutReadEvent::Rejected { record, error } => self.rejected(record, error.kind()),
        }
    }
}

impl p4kx_extraction::CanonicalReadObserver for Capture<'_> {
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
