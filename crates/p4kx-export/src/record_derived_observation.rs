// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::catalog_read::original::RecordSubject;
use crate::export_diagnostic::ExportSourceRole;
use crate::record_observation::SourceObservation;
use crate::record_source_status::SourceDiagnosis;
use crate::selection::Kind;
use crate::{ExportAccountingError, HealthText};

/// One failed derived operation forwarding its already paired original source.
#[derive(Debug, Clone)]
pub(crate) struct DerivedFailure {
    pub(crate) affected_kind: Kind,
    pub(crate) output_kind: Kind,
    pub(crate) record_index: u32,
    pub(crate) role: ExportSourceRole,
    pub(crate) guid: Guid,
    pub(crate) record_path: Option<HealthText>,
    pub(crate) diagnosis: SourceDiagnosis,
    subject: RecordSubject,
}

impl DerivedFailure {
    pub(crate) fn of_source(
        source: &SourceObservation,
        affected_kind: Kind,
        output_kind: Kind,
        role: ExportSourceRole,
        diagnosis: SourceDiagnosis,
    ) -> Result<Self, ExportAccountingError> {
        source.key()?;
        Self::of_subject(
            source.subject().clone(),
            affected_kind,
            output_kind,
            role,
            diagnosis,
        )
    }

    pub(crate) fn of_subject(
        subject: RecordSubject,
        affected_kind: Kind,
        output_kind: Kind,
        role: ExportSourceRole,
        diagnosis: SourceDiagnosis,
    ) -> Result<Self, ExportAccountingError> {
        let key = subject
            .key()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        Ok(Self {
            affected_kind,
            output_kind,
            record_index: key.index(),
            role,
            guid: key.guid(),
            record_path: key.path_text().cloned(),
            diagnosis,
            subject,
        })
    }

    pub(crate) fn check(&self) -> Result<(), ExportAccountingError> {
        let key = self
            .subject
            .key()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        if key.index() != self.record_index
            || key.guid() != self.guid
            || key.path() != self.record_path.as_ref().map(HealthText::as_str)
        {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(())
    }
    pub(crate) const fn subject(&self) -> &RecordSubject {
        &self.subject
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        self.check()?;
        other.check()?;
        if self != &other {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.diagnosis.merge(other.diagnosis)?;
        self.subject.merge(other.subject)
    }
}

// Accounting equality never substitutes for the separate original-subject merge.
impl PartialEq for DerivedFailure {
    fn eq(&self, other: &Self) -> bool {
        self.affected_kind == other.affected_kind
            && self.output_kind == other.output_kind
            && self.record_index == other.record_index
            && self.role == other.role
            && self.guid == other.guid
            && self.record_path == other.record_path
            && self.diagnosis == other.diagnosis
    }
}

impl Eq for DerivedFailure {}
