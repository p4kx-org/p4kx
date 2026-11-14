// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::catalog_read::original::{ExampleBodyReceipt, RecordSubject};
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::health_diagnostic_observation::ExampleObservation;
use crate::{CatalogError, ExportAccountingError};
/// The exact source disposition recorded where a reader or projection finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceDisposition {
    /// A canonical read and typed projection completed, including zero-row projections.
    Projected,
    /// A documented policy excluded the Record before a canonical read.
    SkippedBeforeRead,
    /// A documented policy excluded the Record after a successful canonical read.
    SkippedAfterRead,
    /// The canonical source could not be read.
    ReadFailed,
    /// The canonical source was read but its typed projection failed.
    ProjectionFailed,
}

impl SourceDisposition {
    #[must_use]
    pub(crate) const fn read_succeeded(self) -> bool {
        matches!(
            self,
            Self::Projected | Self::SkippedAfterRead | Self::ProjectionFailed
        )
    }

    #[must_use]
    pub(crate) const fn failed(self) -> bool {
        matches!(self, Self::ReadFailed | Self::ProjectionFailed)
    }
}

/// Available technical field metadata of a source failure.
#[derive(Debug, Clone, Default)]
pub(crate) struct SourceDiagnosis {
    pub(crate) code: Option<ExportDiagnosticCode>,
    pub(crate) field: Option<String>,
    pub(crate) source_type: Option<String>,
    pub(crate) target_guid: Option<Guid>,
    pub(crate) body: DiagnosisBody,
}

/// The actual creator's body recipe; pending captures cannot enter a canonical report.
#[derive(Debug, Clone, Default)]
pub(crate) enum DiagnosisBody {
    #[default]
    Absent,
    Pending,
    Conflicting,
    Captured(ExampleBodyReceipt),
}

impl SourceDiagnosis {
    pub(crate) fn from_body(
        code: ExportDiagnosticCode,
        body: ExampleBodyReceipt,
    ) -> Result<Self, CatalogError> {
        let (field, source_type, target_guid) = body.description()?;
        Ok(Self {
            code: Some(code),
            field: field.as_deref().map(str::to_owned),
            source_type: source_type.as_deref().map(str::to_owned),
            target_guid,
            body: DiagnosisBody::Captured(body),
        })
    }

    pub(crate) fn example(
        &self,
        subject: RecordSubject,
    ) -> Result<ExampleObservation, ExportAccountingError> {
        let body = match &self.body {
            DiagnosisBody::Absent
                if self.field.is_none()
                    && self.source_type.is_none()
                    && self.target_guid.is_none() =>
            {
                ExampleBodyReceipt::absent(subject.clone())
            }
            DiagnosisBody::Absent | DiagnosisBody::Pending => {
                return Ok(ExampleObservation::new(
                    subject,
                    self.field.as_deref(),
                    self.source_type.as_deref(),
                    self.target_guid,
                ));
            }
            DiagnosisBody::Captured(body) => body.clone(),
            DiagnosisBody::Conflicting => {
                return Err(ExportAccountingError::ConflictingSourceDatum);
            }
        };
        ExampleObservation::captured(
            subject,
            self.field.as_deref(),
            self.source_type.as_deref(),
            self.target_guid,
            body,
        )
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if self != &other {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        match (&mut self.body, other.body) {
            (DiagnosisBody::Absent, DiagnosisBody::Absent)
            | (DiagnosisBody::Pending, DiagnosisBody::Pending)
            | (DiagnosisBody::Conflicting, DiagnosisBody::Conflicting) => Ok(()),
            (DiagnosisBody::Captured(body), DiagnosisBody::Captured(other)) => body.merge(other),
            _ => Err(ExportAccountingError::UnboundHealthSubject),
        }
    }
}

impl PartialEq for SourceDiagnosis {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
            && self.field == other.field
            && self.source_type == other.source_type
            && self.target_guid == other.target_guid
    }
}
impl Eq for SourceDiagnosis {}
