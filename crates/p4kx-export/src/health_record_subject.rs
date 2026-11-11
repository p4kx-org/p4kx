// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Guid, Record, SourceUses};

use super::OriginalDatum;
use super::health_header_receipt::{CapturedSubject, HeaderReadReceipt};
use super::health_record_capture::CapturedRecord;
use super::health_record_key::RecordSubjectKey;
use super::health_text_capture::CapturedText;
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError, HealthText};

#[derive(Debug, Clone)]
enum Subject {
    Captured(Arc<HeaderReadReceipt>),
    Accounting,
    Absent,
}

/// A retained original header read or an explicit non-authorizing accounting state.
#[derive(Debug, Clone)]
pub(crate) struct RecordSubject {
    key: Option<RecordSubjectKey>,
    subject: Subject,
}

impl RecordSubject {
    pub(crate) fn capture(
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
    ) -> Result<Self, CatalogError> {
        let facts = HeaderReadReceipt::read(input, record)?;
        let key = RecordSubjectKey::new(
            facts.index(),
            facts.guid(),
            facts.path().map(|path| path.text().clone()),
        );
        Ok(Self {
            key: Some(key),
            subject: Subject::Captured(Arc::new(facts)),
        })
    }

    pub(crate) const fn accounting(index: u32, guid: Guid, path: Option<HealthText>) -> Self {
        Self {
            key: Some(RecordSubjectKey::new(index, guid, path)),
            subject: Subject::Accounting,
        }
    }

    pub(crate) const fn absent() -> Self {
        Self {
            key: None,
            subject: Subject::Absent,
        }
    }

    pub(crate) const fn key(&self) -> Option<&RecordSubjectKey> {
        self.key.as_ref()
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if self.key != other.key {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        match (&mut self.subject, other.subject) {
            (Subject::Captured(retained), Subject::Captured(other)) => {
                retained.merge(&other)?;
            }
            (Subject::Accounting, Subject::Accounting) | (Subject::Absent, Subject::Absent) => {}
            _ => return Err(ExportAccountingError::UnboundHealthSubject),
        }
        Ok(())
    }

    pub(crate) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        match &self.subject {
            Subject::Captured(receipt) => receipt.check_input(input),
            Subject::Absent => Ok(()),
            Subject::Accounting => Err(ExportAccountingError::UnboundHealthSubject),
        }
    }
    pub(super) fn retain_guid(
        &self,
        inventory: &SourceUses,
    ) -> Result<OriginalDatum<Guid>, CatalogError> {
        match &self.subject {
            Subject::Captured(receipt) => receipt.guid_only(inventory),
            Subject::Accounting | Subject::Absent => {
                Err(ExportAccountingError::UnboundHealthSubject.into())
            }
        }
    }

    pub(super) fn source_type_text(&self) -> Result<Option<HealthText>, ExportAccountingError> {
        match &self.subject {
            Subject::Captured(receipt) => Ok(receipt.source_type_text().cloned()),
            Subject::Accounting | Subject::Absent => {
                Err(ExportAccountingError::UnboundHealthSubject)
            }
        }
    }

    pub(super) fn retain_source_type(
        &self,
        inventory: &SourceUses,
    ) -> Result<OriginalDatum<CapturedText>, CatalogError> {
        match &self.subject {
            Subject::Captured(receipt) => receipt.source_type_only(inventory),
            Subject::Accounting | Subject::Absent => {
                Err(ExportAccountingError::UnboundHealthSubject.into())
            }
        }
    }

    pub(super) fn retain_record(
        &self,
        inventory: &SourceUses,
    ) -> Result<CapturedRecord, CatalogError> {
        match &self.subject {
            Subject::Captured(receipt) => receipt.record(inventory),
            Subject::Accounting | Subject::Absent => {
                Err(ExportAccountingError::UnboundHealthSubject.into())
            }
        }
    }

    pub(super) fn retain_diagnostic(
        &self,
        inventory: &SourceUses,
    ) -> Result<Option<CapturedSubject>, CatalogError> {
        match &self.subject {
            Subject::Captured(receipt) => receipt.subject(inventory).map(Some),
            Subject::Absent => Ok(None),
            Subject::Accounting => Err(ExportAccountingError::UnboundHealthSubject.into()),
        }
    }
}
