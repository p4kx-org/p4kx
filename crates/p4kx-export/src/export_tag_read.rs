// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, Record, TagBuildEvent, TagRelationIssue};

use crate::build::original_input::OriginalCatalogInput;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::health_diagnostic_observation::{
    DiagnosticObservation as ExportDiagnostic, ExampleObservation,
};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, CatalogReadFailure, Kind};

pub(crate) struct Capture<'context, 'f> {
    input: Option<&'context OriginalCatalogInput<'context, 'f>>,
    kind: Kind,
    ledger: ExportLedger,
    failures: Vec<CatalogReadFailure<'f>>,
}

impl<'context, 'f> Capture<'context, 'f> {
    pub(crate) fn new(
        kind: Kind,
        input: Option<&'context OriginalCatalogInput<'context, 'f>>,
    ) -> Self {
        Self {
            input,
            kind,
            ledger: ExportLedger::default(),
            failures: Vec::new(),
        }
    }

    pub(crate) fn finish(self) -> (Vec<CatalogReadFailure<'f>>, ExportLedger) {
        (self.failures, self.ledger)
    }
}

impl<'f> Capture<'_, 'f> {
    fn admission<'read>(
        &self,
        record: &'read Record<'f>,
    ) -> Result<RecordObservation<'read, 'f>, CatalogError> {
        self.input.map_or_else(
            || Ok(RecordObservation::accounting(record)),
            |input| RecordObservation::capture(input, record),
        )
    }
    fn observe(
        &mut self,
        record: &Record<'f>,
        disposition: SourceDisposition,
    ) -> Result<(), CatalogError> {
        let admission = self.admission(record)?;
        self.ledger.observe(admission.source(
            Some(self.kind),
            ExportSourceRole::Dependency,
            disposition,
        )?);
        Ok(())
    }

    fn failed(
        &mut self,
        record: &Record<'f>,
        error: &DataForgeError,
        role: ExportSourceRole,
    ) -> Result<(), CatalogError> {
        let admission = self.admission(record)?;
        self.ledger.observe(
            admission
                .source(Some(self.kind), role, SourceDisposition::ReadFailed)?
                .diagnosed(crate::export_source_failure::diagnosis(error)),
        );
        self.failures.push(CatalogReadFailure::of(record, error));
        Ok(())
    }

    pub(crate) fn event(&mut self, event: &TagBuildEvent<'_, 'f>) -> Result<(), CatalogError> {
        match event {
            TagBuildEvent::CanonicalRead {
                record,
                outcome: Err(error),
            } => self.failed(record, error, ExportSourceRole::Dependency),
            TagBuildEvent::MetadataFailed { record, error } => {
                self.failed(record, error, ExportSourceRole::SelectionCandidate)
            }
            TagBuildEvent::Projected { record } => {
                self.observe(record, SourceDisposition::Projected)
            }
            TagBuildEvent::DuplicateSkipped { record } => {
                self.observe(record, SourceDisposition::SkippedAfterRead)
            }
            TagBuildEvent::CanonicalRead { outcome: Ok(_), .. }
            | TagBuildEvent::RelationSkipped { .. }
            | TagBuildEvent::PathChosen { .. } => Ok(()),
            TagBuildEvent::Cancelled => Err(CatalogError::Cancelled),
        }
    }

    pub(crate) fn relation(&mut self, reason: TagRelationIssue, example: ExampleObservation) {
        let (code, severity) = match reason {
            TagRelationIssue::MalformedValue => (
                ExportDiagnosticCode::MalformedSource,
                ExportDiagnosticSeverity::Warning,
            ),
            TagRelationIssue::Dangling => (
                ExportDiagnosticCode::DanglingReference,
                ExportDiagnosticSeverity::Information,
            ),
            TagRelationIssue::Repeated => (
                ExportDiagnosticCode::SourceWarning,
                ExportDiagnosticSeverity::Information,
            ),
        };
        self.ledger.field_diagnostic(ExportDiagnostic {
            kind: Some(self.kind),
            stage: ExportDiagnosticStage::SourceField,
            code,
            severity,
            source_role: Some(ExportSourceRole::Dependency),
            warning_code: None,
            count: 1,
            count_unit: ExportDiagnosticUnit::Values,
            examples: vec![example],
        });
    }
}

#[cfg(test)]
#[path = "export_tag_read_standalone.rs"]
mod standalone;

#[cfg(test)]
pub(crate) use standalone::read;
