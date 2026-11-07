// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Record, RecordTree, TagBuildEvent,
    TagProjectionFailure, TagRelationIssue, TagTree,
};

use crate::export_diagnostic::{
    ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticExample, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole,
};
use crate::export_ledger::{ExportLedger, SourceDiagnosis, SourceDisposition, SourceObservation};
use crate::{CatalogError, CatalogFailure, Kind};

#[derive(Debug, thiserror::Error)]
#[error("tag tree projection rejected {0:?}")]
struct ProjectionFailure(TagProjectionFailure);

pub(crate) struct TagRead {
    pub(crate) tree: TagTree,
    pub(crate) ledger: ExportLedger,
    pub(crate) failures: Vec<CatalogFailure>,
}

struct Capture {
    kind: Kind,
    ledger: ExportLedger,
    failures: Vec<CatalogFailure>,
}

impl Capture {
    fn observe(&mut self, record: &Record<'_>, disposition: SourceDisposition) {
        self.ledger.observe(SourceObservation::new(
            self.kind,
            record,
            ExportSourceRole::Dependency,
            disposition,
        ));
    }

    fn failed(&mut self, record: &Record<'_>, error: &DataForgeError, role: ExportSourceRole) {
        self.ledger.observe(
            SourceObservation::new(self.kind, record, role, SourceDisposition::ReadFailed)
                .diagnosed(crate::export_source_failure::diagnosis(error)),
        );
        self.failures.push(CatalogFailure::of(record, error));
    }

    fn canonical(
        &mut self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        match outcome {
            Ok(tree) => self.ledger.merge(crate::export_field_walk::inspect(
                tree,
                record,
                Some(self.kind),
                ExportSourceRole::Dependency,
                cancel,
            )?),
            Err(error) => self.failed(record, error, ExportSourceRole::Dependency),
        }
        Ok(())
    }

    fn event(
        &mut self,
        event: &TagBuildEvent<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        match event {
            TagBuildEvent::CanonicalRead { record, outcome } => {
                self.canonical(record, *outcome, cancel)?;
            }
            TagBuildEvent::MetadataFailed { record, error } => {
                self.failed(record, error, ExportSourceRole::SelectionCandidate);
            }
            TagBuildEvent::Projected { record } => {
                self.observe(record, SourceDisposition::Projected);
            }
            TagBuildEvent::DuplicateSkipped { record } => {
                self.observe(record, SourceDisposition::SkippedAfterRead);
            }
            TagBuildEvent::ProjectionFailed { record, reason } => {
                self.projection_failure(record, *reason);
            }
            TagBuildEvent::RelationSkipped {
                record,
                field,
                target,
                reason,
            } => relation(&mut self.ledger, self.kind, record, field, *target, *reason),
            TagBuildEvent::Cancelled => return Err(CatalogError::Cancelled),
        }
        Ok(())
    }

    fn projection_failure(&mut self, record: &Record<'_>, reason: TagProjectionFailure) {
        let field = if reason == TagProjectionFailure::NameBytes {
            "tagName"
        } else {
            "tagPath"
        };
        self.ledger.observe(
            SourceObservation::new(
                self.kind,
                record,
                ExportSourceRole::Dependency,
                SourceDisposition::ProjectionFailed,
            )
            .diagnosed(SourceDiagnosis {
                code: Some(ExportDiagnosticCode::ProjectionFailure),
                field: Some(field.to_owned()),
                source_type: Some("Tag".to_owned()),
                target_guid: None,
            }),
        );
        self.failures
            .push(CatalogFailure::of(record, &ProjectionFailure(reason)));
    }
}

fn relation(
    ledger: &mut ExportLedger,
    kind: Kind,
    record: &Record<'_>,
    field: &str,
    target: Option<p4kx_extraction::Guid>,
    reason: TagRelationIssue,
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
        examples: vec![ExportDiagnosticExample {
            record_index: Some(record.index()),
            record_guid: Some(record.guid().to_string()),
            record_path: record.record_path().ok().map(str::to_owned),
            field: Some(field.to_owned()),
            source_type: record.type_name().ok().map(str::to_owned),
            target_guid: target.map(|guid| guid.to_string()),
        }],
    });
}

pub(crate) fn read(
    forge: &DataForge<'_>,
    kind: Kind,
    cancel: Option<&CancelToken>,
) -> Result<TagRead, CatalogError> {
    let mut capture = Capture {
        kind,
        ledger: ExportLedger::default(),
        failures: Vec::new(),
    };
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
