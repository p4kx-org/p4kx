// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Record, RecordTree, TagBuildEvent, TagPathEdgeKind,
    TagRelationIssue, TagTree,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, TagRelationReceipts};
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

pub(crate) struct TagRead<'f> {
    pub(crate) tree: TagTree,
    pub(crate) ledger: ExportLedger,
    pub(crate) failures: Vec<CatalogReadFailure<'f>>,
}

pub(crate) struct Capture<'context, 'f> {
    input: Option<&'context OriginalCatalogInput<'context, 'f>>,
    kind: Kind,
    ledger: ExportLedger,
    failures: Vec<CatalogReadFailure<'f>>,
    relations: TagRelationReceipts,
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
            relations: TagRelationReceipts::default(),
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

    fn canonical(
        &mut self,
        record: &Record<'f>,
        outcome: Result<&RecordTree<'f>, &DataForgeError>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let admission = self.admission(record)?;
        match outcome {
            Ok(tree) => {
                if let Some(input) = self.input {
                    let scope = input.record_scope(record, tree)?;
                    let original = OriginalFlowFields::read(&scope, cancel)?;
                    let body = original.body_capture(input, &admission)?;
                    let kind = if tree.type_name() == "TagDatabase" {
                        TagPathEdgeKind::Root
                    } else {
                        TagPathEdgeKind::Child
                    };
                    self.relations.capture(kind, tree, &original, &body)?;
                }
                self.ledger
                    .merge(crate::export_field_walk::inspect_observed(
                        self.input,
                        tree,
                        &admission,
                        Some(self.kind),
                        ExportSourceRole::Dependency,
                        cancel,
                    )?);
            }
            Err(error) => self.failed(record, error, ExportSourceRole::Dependency)?,
        }
        Ok(())
    }

    pub(crate) fn event(
        &mut self,
        event: &TagBuildEvent<'_, 'f>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        match event {
            TagBuildEvent::CanonicalRead { record, outcome } => {
                self.canonical(record, *outcome, cancel)?;
            }
            TagBuildEvent::MetadataFailed { record, error } => {
                self.failed(record, error, ExportSourceRole::SelectionCandidate)?;
            }
            TagBuildEvent::Projected { record } => {
                self.observe(record, SourceDisposition::Projected)?;
            }
            TagBuildEvent::DuplicateSkipped { record } => {
                self.observe(record, SourceDisposition::SkippedAfterRead)?;
            }
            TagBuildEvent::RelationSkipped {
                incoming,
                target,
                reason,
            } => {
                let admission = self.admission(incoming.record())?;
                let example = if let Some(input) = self.input {
                    let body =
                        self.relations
                            .take(input, admission.subject(), *incoming, *target)?;
                    let (field, source_type, target_guid) = body.description()?;
                    ExampleObservation::captured(
                        admission.subject().clone(),
                        field.as_deref(),
                        source_type.as_deref(),
                        target_guid,
                        body,
                    )?
                } else {
                    ExampleObservation::new(
                        admission.subject().clone(),
                        Some(incoming.kind().field()),
                        admission.record().type_name().ok(),
                        *target,
                    )
                };
                relation(&mut self.ledger, self.kind, *reason, example);
            }
            TagBuildEvent::PathChosen { .. } => {}
            TagBuildEvent::Cancelled => return Err(CatalogError::Cancelled),
        }
        Ok(())
    }
}

fn relation(
    ledger: &mut ExportLedger,
    kind: Kind,
    reason: TagRelationIssue,
    example: ExampleObservation,
) {
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
    ledger.field_diagnostic(ExportDiagnostic {
        kind: Some(kind),
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

pub(crate) fn read<'f>(
    forge: &'f DataForge<'f>,
    kind: Kind,
    input: Option<&OriginalCatalogInput<'_, 'f>>,
    cancel: Option<&CancelToken>,
) -> Result<TagRead<'f>, CatalogError> {
    let mut capture = Capture::new(kind, input);
    let tree = TagTree::build_observed(
        forge,
        &|| cancel.is_some_and(CancelToken::is_cancelled),
        &mut |event| capture.event(&event, cancel),
    )?
    .ok_or(CatalogError::Cancelled)?;
    Ok(TagRead {
        tree,
        ledger: capture.ledger,
        failures: capture.failures,
    })
}
