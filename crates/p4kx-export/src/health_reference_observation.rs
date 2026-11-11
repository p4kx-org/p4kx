// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::Guid;

use super::RecordSubject;
use super::health_reference_data::ReferenceData;
use super::health_reference_receipt::{ReferenceDatum, ReferenceFrame};
use crate::{ExportAccountingError, ExportReferenceStatus, ExportSourceRole, HealthText, Kind};

/// Retained source recipes for one actual Reference occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReferenceBody {
    pub(super) field: ReferenceDatum<HealthText>,
    pub(super) source_type: ReferenceDatum<HealthText>,
    pub(super) target_guid: ReferenceDatum<Guid>,
    pub(super) target_record_index: ReferenceDatum<u32>,
    pub(super) target_record_path: ReferenceDatum<HealthText>,
    pub(super) target_type: ReferenceDatum<HealthText>,
    pub(super) target_location: ReferenceDatum<HealthText>,
    pub(super) source_pointer: Option<ReferencePointerBody>,
}

/// Original raw pointer-word recipes carried by an applicable failed pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReferencePointerBody {
    pub(super) structure: ReferenceDatum<u32>,
    pub(super) instance: ReferenceDatum<u32>,
    pub(super) declared_type: ReferenceDatum<u32>,
}

#[derive(Debug, Clone)]
pub(super) enum ReferenceEvidence {
    Captured {
        frame: Arc<ReferenceFrame>,
        body: Arc<ReferenceBody>,
    },
    Accounting(RecordSubject),
}

/// Private unsampled accounting paired to its actual read or explicit unbound interpretation.
#[derive(Debug, Clone)]
pub(crate) struct ReferenceObservation {
    pub(crate) kind: Option<Kind>,
    pub(crate) source_role: ExportSourceRole,
    data: ReferenceData,
    status: Option<ExportReferenceStatus>,
    pub(super) evidence: ReferenceEvidence,
}

impl ReferenceObservation {
    pub(super) fn captured(
        data: ReferenceData,
        status: Option<ExportReferenceStatus>,
        frame: Arc<ReferenceFrame>,
        body: ReferenceBody,
    ) -> Self {
        Self {
            kind: None,
            source_role: ExportSourceRole::Dependency,
            data,
            status,
            evidence: ReferenceEvidence::Captured {
                frame,
                body: Arc::new(body),
            },
        }
    }

    pub(crate) const fn accounting(
        subject: RecordSubject,
        data: ReferenceData,
        status: Option<ExportReferenceStatus>,
    ) -> Self {
        Self {
            kind: None,
            source_role: ExportSourceRole::Dependency,
            data,
            status,
            evidence: ReferenceEvidence::Accounting(subject),
        }
    }

    pub(crate) const fn data(&self) -> &ReferenceData {
        &self.data
    }

    pub(crate) const fn status(&self) -> Option<ExportReferenceStatus> {
        self.status
    }

    pub(crate) const fn resolved(mut self, status: ExportReferenceStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub(crate) const fn contextualize(&mut self, kind: Option<Kind>, role: ExportSourceRole) {
        self.kind = kind;
        self.source_role = role;
    }

    pub(crate) fn subject(&self) -> &RecordSubject {
        match &self.evidence {
            ReferenceEvidence::Captured { frame, .. } => frame.subject(),
            ReferenceEvidence::Accounting(subject) => subject,
        }
    }

    pub(crate) fn merge_owner(&self, other: &RecordSubject) -> Result<(), ExportAccountingError> {
        let mut subject = self.subject().clone();
        subject.merge(other.clone())
    }

    pub(crate) fn merge(&self, other: Self) -> Result<(), ExportAccountingError> {
        if self != &other {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        match (&self.evidence, other.evidence) {
            (
                ReferenceEvidence::Captured { frame, body },
                ReferenceEvidence::Captured {
                    frame: other,
                    body: other_body,
                },
            ) => {
                if body != &other_body {
                    return Err(ExportAccountingError::ConflictingSourceDatum);
                }
                let mut original = frame.as_ref().clone();
                original.merge(other.as_ref().clone())
            }
            (ReferenceEvidence::Accounting(subject), ReferenceEvidence::Accounting(other)) => {
                let mut original = subject.clone();
                original.merge(other)
            }
            _ => Err(ExportAccountingError::UnboundHealthSubject),
        }
    }
}

impl PartialEq for ReferenceObservation {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.source_role == other.source_role
            && self.status == other.status
            && self.data == other.data
            && self.subject().key() == other.subject().key()
    }
}
impl Eq for ReferenceObservation {}
