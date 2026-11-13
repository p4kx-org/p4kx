// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Record, RecordTree, SourceEvidenceError};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{RecordSubject, RecordSubjectKey};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::ConfigurationInspection;
use crate::record_source_status::{SourceDiagnosis, SourceDisposition};
use crate::selection::Kind;
use crate::{CatalogError, ExportAccountingError, HealthText};

/// One actual Record admission retained across its read and projection outcomes.
pub(crate) struct RecordObservation<'read, 'source> {
    record: &'read Record<'source>,
    subject: RecordSubject,
}

impl<'read, 'source> RecordObservation<'read, 'source> {
    pub(crate) fn capture(
        input: &OriginalCatalogInput<'_, '_>,
        record: &'read Record<'source>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            record,
            subject: RecordSubject::capture(input, record)?,
        })
    }

    pub(crate) fn accounting(record: &'read Record<'source>) -> Self {
        Self {
            record,
            subject: RecordSubject::accounting(
                record.index(),
                record.guid(),
                record.record_path().ok().map(HealthText::new),
            ),
        }
    }

    pub(crate) const fn record(&self) -> &'read Record<'source> {
        self.record
    }

    pub(crate) const fn subject(&self) -> &RecordSubject {
        &self.subject
    }

    pub(crate) fn check_tree(&self, tree: &RecordTree<'_>) -> Result<(), CatalogError> {
        if !tree.is_from(self.record) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }

    pub(crate) fn source(
        &self,
        kind: Option<Kind>,
        role: ExportSourceRole,
        disposition: SourceDisposition,
    ) -> Result<SourceObservation, CatalogError> {
        let key = self
            .subject
            .key()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        Ok(SourceObservation {
            kind,
            record_index: key.index(),
            guid: key.guid(),
            record_path: key.path_text().cloned(),
            role,
            disposition,
            diagnosis: SourceDiagnosis::default(),
            configuration: None,
            subject: self.subject.clone(),
        })
    }
}

/// Immutable physical accounting paired with its original admission or explicit interpreted state.
#[derive(Debug, Clone)]
#[must_use]
pub(crate) struct SourceObservation {
    pub(crate) kind: Option<Kind>,
    pub(crate) record_index: u32,
    pub(crate) guid: Guid,
    pub(crate) record_path: Option<HealthText>,
    pub(crate) role: ExportSourceRole,
    pub(crate) disposition: SourceDisposition,
    pub(crate) diagnosis: SourceDiagnosis,
    pub(crate) configuration: Option<ConfigurationInspection>,
    subject: RecordSubject,
}

impl SourceObservation {
    #[cfg(test)]
    pub(crate) fn accounting(
        kind: Option<Kind>,
        index: u32,
        guid: Guid,
        path: Option<HealthText>,
        role: ExportSourceRole,
        disposition: SourceDisposition,
    ) -> Self {
        Self {
            kind,
            record_index: index,
            guid,
            record_path: path.clone(),
            role,
            disposition,
            diagnosis: SourceDiagnosis::default(),
            configuration: None,
            subject: RecordSubject::accounting(index, guid, path),
        }
    }
    #[cfg(test)]
    /// Records interpreted accounting without original-header authority.
    pub(crate) fn new(
        kind: Kind,
        record: &Record<'_>,
        role: ExportSourceRole,
        disposition: SourceDisposition,
    ) -> Self {
        Self::catalog_source(record, role, disposition).with_kind(kind)
    }

    #[cfg(test)]
    /// Records interpreted accounting without claiming a catalog source binding.
    pub(crate) fn catalog_source(
        record: &Record<'_>,
        role: ExportSourceRole,
        disposition: SourceDisposition,
    ) -> Self {
        Self {
            kind: None,
            record_index: record.index(),
            guid: record.guid(),
            record_path: record.record_path().ok().map(HealthText::new),
            role,
            disposition,
            diagnosis: SourceDiagnosis::default(),
            configuration: None,
            subject: RecordObservation::accounting(record).subject,
        }
    }

    #[cfg(test)]
    const fn with_kind(mut self, kind: Kind) -> Self {
        self.kind = Some(kind);
        self
    }
    pub(crate) const fn for_configuration(
        mut self,
        configuration: ConfigurationInspection,
    ) -> Self {
        self.configuration = Some(configuration);
        self
    }

    pub(crate) fn diagnosed(mut self, diagnosis: SourceDiagnosis) -> Self {
        self.diagnosis = diagnosis;
        self
    }

    pub(crate) const fn subject(&self) -> &RecordSubject {
        &self.subject
    }

    pub(crate) fn into_subject(self) -> RecordSubject {
        self.subject
    }
    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if self != &other {
            return Err(ExportAccountingError::ConflictingDisposition { kind: self.kind });
        }
        self.diagnosis.merge(other.diagnosis)?;
        self.subject.merge(other.subject)
    }

    pub(crate) fn key(&self) -> Result<&RecordSubjectKey, ExportAccountingError> {
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
        Ok(key)
    }
}

// Equality describes accounting values; merge retains and validates the separate live proof.
impl PartialEq for SourceObservation {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.record_index == other.record_index
            && self.guid == other.guid
            && self.record_path == other.record_path
            && self.role == other.role
            && self.disposition == other.disposition
            && self.diagnosis == other.diagnosis
            && self.configuration == other.configuration
    }
}

impl Eq for SourceObservation {}
