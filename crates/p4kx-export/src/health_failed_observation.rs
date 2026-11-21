// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::ExportAccountingError;
use crate::catalog_read::original::{RecordSubject, RecordSubjectKey};
use crate::export_diagnostic::{ExportDiagnosticCode, ExportDiagnosticStage, ExportSourceRole};
use crate::selection::Kind;

/// A failed physical identity paired before public report serialization.
#[derive(Debug, Clone)]
pub(crate) struct FailedRecordObservation {
    pub(crate) kind: Option<Kind>,
    pub(crate) role: ExportSourceRole,
    pub(crate) stage: ExportDiagnosticStage,
    pub(crate) code: ExportDiagnosticCode,
    key: RecordSubjectKey,
    subject: RecordSubject,
}

impl FailedRecordObservation {
    pub(crate) fn new(
        kind: Option<Kind>,
        subject: RecordSubject,
        role: ExportSourceRole,
        stage: ExportDiagnosticStage,
        code: ExportDiagnosticCode,
    ) -> Result<Self, ExportAccountingError> {
        let key = subject
            .key()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?
            .clone();
        Ok(Self {
            kind,
            role,
            stage,
            code,
            key,
            subject,
        })
    }

    pub(crate) const fn key(&self) -> &RecordSubjectKey {
        &self.key
    }

    pub(crate) const fn subject(&self) -> &RecordSubject {
        &self.subject
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if self != &other {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.subject.merge(other.subject)
    }
}

// Original authority remains in subject and is merged independently of accounting equality.
impl PartialEq for FailedRecordObservation {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.role == other.role
            && self.stage == other.stage
            && self.code == other.code
            && self.key == other.key
    }
}

impl Eq for FailedRecordObservation {}
