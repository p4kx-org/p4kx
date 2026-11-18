// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Guid, Record, SourceUse, SourceUses};

use super::OriginalDatum;
use super::health_header_receipt::{CapturedSubject, HeaderReadReceipt};
use super::health_record_capture::CapturedRecord;
use super::health_record_key::RecordSubjectKey;
use super::health_text_capture::CapturedText;
use crate::build::original_input::OriginalCatalogInput;
use crate::selection::KindRule;
use crate::{CatalogError, ExportAccountingError, HealthText};

#[derive(Debug, Clone)]
enum Subject {
    Captured(Arc<HeaderReadReceipt>),
    Accounting(Arc<RecordSubjectKey>),
    Absent,
}

/// A retained original header read or an explicit non-authorizing accounting state.
#[derive(Debug, Clone)]
pub(crate) struct RecordSubject {
    subject: Subject,
}

impl RecordSubject {
    pub(crate) fn capture(
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
    ) -> Result<Self, CatalogError> {
        Ok(Self::from_receipt(HeaderReadReceipt::read(input, record)?))
    }

    /// Captures complete header receipts when the checked selection yields an outcome.
    pub(crate) fn capture_selected<T>(
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
        choose: impl FnOnce() -> Option<T>,
    ) -> Result<Option<(Self, T)>, CatalogError> {
        let selected = HeaderReadReceipt::read_selected(input, record, choose)?;
        Ok(selected.map(|(facts, selection)| (Self::from_receipt(facts), selection)))
    }

    fn from_receipt(facts: HeaderReadReceipt) -> Self {
        Self {
            subject: Subject::Captured(Arc::new(facts)),
        }
    }

    pub(crate) fn accounting(index: u32, guid: Guid, path: Option<HealthText>) -> Self {
        Self {
            subject: Subject::Accounting(Arc::new(RecordSubjectKey::new(index, guid, path))),
        }
    }

    pub(crate) const fn absent() -> Self {
        Self {
            subject: Subject::Absent,
        }
    }

    pub(crate) fn key(&self) -> Option<&RecordSubjectKey> {
        match &self.subject {
            Subject::Captured(receipt) => Some(receipt.key()),
            Subject::Accounting(key) => Some(key),
            Subject::Absent => None,
        }
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if self.key() != other.key() {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        match (&mut self.subject, other.subject) {
            (Subject::Captured(retained), Subject::Captured(other)) => {
                retained.merge(&other)?;
            }
            (Subject::Accounting(_), Subject::Accounting(_))
            | (Subject::Absent, Subject::Absent) => {}
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
            Subject::Accounting(_) => Err(ExportAccountingError::UnboundHealthSubject),
        }
    }
    /// Retains original inputs when readable headers cannot classify a possible rule source.
    pub(crate) fn unclassified_selection_inputs(
        &self,
        rule: KindRule,
        inventory: &SourceUses,
    ) -> Result<Option<Vec<SourceUse>>, CatalogError> {
        match &self.subject {
            Subject::Captured(receipt) => receipt.unclassified_selection_inputs(rule, inventory),
            Subject::Accounting(_) | Subject::Absent => {
                Err(ExportAccountingError::UnboundHealthSubject.into())
            }
        }
    }
    pub(super) fn retain_guid(
        &self,
        inventory: &SourceUses,
    ) -> Result<OriginalDatum<Guid>, CatalogError> {
        match &self.subject {
            Subject::Captured(receipt) => receipt.guid_only(inventory),
            Subject::Accounting(_) | Subject::Absent => {
                Err(ExportAccountingError::UnboundHealthSubject.into())
            }
        }
    }

    pub(super) fn source_type_text(&self) -> Result<Option<HealthText>, ExportAccountingError> {
        match &self.subject {
            Subject::Captured(receipt) => Ok(receipt.source_type_text().cloned()),
            Subject::Accounting(_) | Subject::Absent => {
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
            Subject::Accounting(_) | Subject::Absent => {
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
            Subject::Accounting(_) | Subject::Absent => {
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
            Subject::Accounting(_) => Err(ExportAccountingError::UnboundHealthSubject.into()),
        }
    }
}
