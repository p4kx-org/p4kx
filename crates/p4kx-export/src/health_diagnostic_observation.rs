// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use p4kx_extraction::Guid;

use crate::catalog_read::original::{ExampleBodyReceipt, RecordSubject, RecordSubjectKey};
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::guid_text::CatalogGuid;
use crate::selection::Kind;
use crate::{ExportAccountingError, HealthText};

/// A diagnostic subject retained before report-local text positions are assigned.
#[derive(Debug, Clone)]
pub(crate) struct ExampleObservation {
    subject: RecordSubject,
    body: Option<ExampleBodyReceipt>,
    pub(crate) field: Option<HealthText>,
    pub(crate) source_type: Option<HealthText>,
    pub(crate) target_guid: Option<Guid>,
}

impl ExampleObservation {
    pub(crate) fn new(
        subject: RecordSubject,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
    ) -> Self {
        Self {
            subject,
            body: None,
            field: field.map(HealthText::new),
            source_type: source_type.map(HealthText::new),
            target_guid,
        }
    }

    pub(crate) fn captured(
        subject: RecordSubject,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
        body: ExampleBodyReceipt,
    ) -> Result<Self, ExportAccountingError> {
        body.check_description(field, source_type, target_guid)?;
        let (field, source_type, target_guid) = body.description()?;
        Ok(Self {
            subject,
            body: Some(body),
            field,
            source_type,
            target_guid,
        })
    }

    pub(crate) fn body(&self) -> Result<&ExampleBodyReceipt, ExportAccountingError> {
        let body = self
            .body
            .as_ref()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        body.check_description(
            self.field.as_deref(),
            self.source_type.as_deref(),
            self.target_guid,
        )?;
        Ok(body)
    }

    pub(crate) const fn subject(&self) -> &RecordSubject {
        &self.subject
    }

    pub(crate) fn sample_cmp(&self, other: &Self) -> Ordering {
        let left = self.subject.key();
        let right = other.subject.key();
        left.map(RecordSubjectKey::index)
            .cmp(&right.map(RecordSubjectKey::index))
            .then_with(|| {
                left.map(RecordSubjectKey::sample_guid)
                    .cmp(&right.map(RecordSubjectKey::sample_guid))
            })
            .then_with(|| {
                left.and_then(|key| key.path())
                    .cmp(&right.and_then(|key| key.path()))
            })
            .then_with(|| self.field.cmp(&other.field))
            .then_with(|| self.source_type.cmp(&other.source_type))
            .then_with(|| {
                self.target_guid
                    .map(CatalogGuid)
                    .map(CatalogGuid::text)
                    .cmp(&other.target_guid.map(CatalogGuid).map(CatalogGuid::text))
            })
    }

    pub(crate) fn merge_owner(
        &mut self,
        subject: &RecordSubject,
    ) -> Result<(), ExportAccountingError> {
        self.subject.merge(subject.clone())
    }
    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if self.sample_cmp(&other) != Ordering::Equal {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        match (&mut self.body, other.body) {
            (Some(body), Some(other)) => body.merge(other)?,
            (None, None) => {}
            _ => return Err(ExportAccountingError::UnboundHealthSubject),
        }
        self.subject.merge(other.subject)
    }
}

/// Source diagnostics whose subjects have not passed the public report's single seal.
#[derive(Debug, Clone)]
pub(crate) struct DiagnosticObservation {
    pub(crate) kind: Option<Kind>,
    pub(crate) stage: ExportDiagnosticStage,
    pub(crate) code: ExportDiagnosticCode,
    pub(crate) severity: ExportDiagnosticSeverity,
    pub(crate) source_role: Option<ExportSourceRole>,
    pub(crate) warning_code: Option<String>,
    pub(crate) count: usize,
    pub(crate) count_unit: ExportDiagnosticUnit,
    pub(crate) examples: Vec<ExampleObservation>,
}
// Sample equality is value-only; global aggregation separately merges original frames.
impl PartialEq for ExampleObservation {
    fn eq(&self, other: &Self) -> bool {
        self.sample_cmp(other) == Ordering::Equal
    }
}

impl Eq for ExampleObservation {}

impl PartialOrd for ExampleObservation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExampleObservation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sample_cmp(other)
    }
}
impl PartialEq for DiagnosticObservation {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.stage == other.stage
            && self.code == other.code
            && self.severity == other.severity
            && self.source_role == other.source_role
            && self.warning_code == other.warning_code
            && self.count == other.count
            && self.count_unit == other.count_unit
            && self.examples == other.examples
    }
}

impl Eq for DiagnosticObservation {}
