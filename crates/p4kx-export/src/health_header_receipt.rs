// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Guid, Record, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue,
};

use super::OriginalDatum;
use super::health_record_capture::CapturedRecord;
use super::health_record_key::RecordSubjectKey;
use super::health_text_capture::CapturedText;
use crate::build::original_input::OriginalCatalogInput;
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

#[derive(Debug, Clone)]
struct HeaderFrame {
    scope: SourceScope,
    index: SourceUse,
}

impl HeaderFrame {
    fn read(
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
    ) -> Result<Self, CatalogError> {
        let context = input.record_context(record)?;
        let scope = context.dataforge();
        let index = scope.at(
            RecordHeaderField::RecordIndex.location(record.index()),
            SourceUseRole::Selector,
        )?;
        Ok(Self {
            scope: scope.clone(),
            index,
        })
    }
}

/// Three original subject facts materialized only for a retained diagnostic occurrence.
pub(super) struct CapturedSubject {
    pub(super) record_index: OriginalDatum<u32>,
    pub(super) guid: OriginalDatum<Guid>,
    pub(super) record_path: OriginalDatum<CapturedText>,
}

/// Original header getters and their bound input, without premature payload-use claims.
#[derive(Debug, Clone)]
pub(super) struct HeaderReadReceipt {
    inventory: SourceUses,
    key: RecordSubjectKey,
    path: SourceValue<CapturedText>,
    source_type: SourceValue<CapturedText>,
    frame: HeaderFrame,
}

impl HeaderReadReceipt {
    pub(super) fn read(
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
    ) -> Result<Self, CatalogError> {
        let frame = HeaderFrame::read(input, record)?;
        Self::capture(input, record, frame)
    }

    pub(super) fn read_selected<T>(
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
        choose: impl FnOnce() -> Option<T>,
    ) -> Result<Option<(Self, T)>, CatalogError> {
        let frame = HeaderFrame::read(input, record)?;
        choose()
            .map(|selection| Self::capture(input, record, frame).map(|facts| (facts, selection)))
            .transpose()
    }

    fn capture(
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
        frame: HeaderFrame,
    ) -> Result<Self, CatalogError> {
        let texts = input.origins().health_texts();
        let path = text_value(&record.record_path(), texts)?;
        let key_path = match &path {
            SourceValue::Present(path) => Some(path.text().clone()),
            _ => None,
        };
        Ok(Self {
            inventory: input.origins().uses.clone(),
            key: RecordSubjectKey::new(record.index(), record.guid(), key_path),
            path,
            source_type: text_value(&record.type_name(), texts)?,
            frame,
        })
    }

    pub(super) const fn key(&self) -> &RecordSubjectKey {
        &self.key
    }

    pub(super) fn merge(&self, other: &Self) -> Result<(), ExportAccountingError> {
        self.check_inventory(&other.inventory)?;
        if !std::ptr::eq(
            self.frame.index.description(),
            other.frame.index.description(),
        ) {
            return Err(ExportAccountingError::UnboundHealthSubject);
        }
        if self.key.index() != other.key.index()
            || self.key.guid() != other.key.guid()
            || self.path != other.path
            || self.source_type != other.source_type
        {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(())
    }
    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        self.check_inventory(&input.origins().uses)?;
        if self.frame.scope.source_ref() != input.origins().dataforge.source_ref() {
            return Err(ExportAccountingError::UnboundHealthSubject);
        }
        Ok(())
    }
    pub(super) fn subject(&self, inventory: &SourceUses) -> Result<CapturedSubject, CatalogError> {
        self.check_inventory(inventory)?;
        let inputs = vec![self.frame.index.clone()];
        Ok(CapturedSubject {
            record_index: OriginalDatum::observed(
                SourceValue::Present(self.key.index()),
                inputs,
                true,
            ),
            guid: self.header(
                RecordHeaderField::Guid,
                SourceValue::Present(self.key.guid()),
                SourceUseRole::Payload,
            )?,
            record_path: self.header(
                RecordHeaderField::FileName,
                self.path.clone(),
                SourceUseRole::Conversion,
            )?,
        })
    }

    pub(super) fn guid_only(
        &self,
        inventory: &SourceUses,
    ) -> Result<OriginalDatum<Guid>, CatalogError> {
        self.check_inventory(inventory)?;
        self.header(
            RecordHeaderField::Guid,
            SourceValue::Present(self.key.guid()),
            SourceUseRole::Payload,
        )
    }

    pub(super) const fn source_type_text(&self) -> Option<&crate::HealthText> {
        match &self.source_type {
            SourceValue::Present(value) => Some(value.text()),
            _ => None,
        }
    }

    pub(super) fn source_type_only(
        &self,
        inventory: &SourceUses,
    ) -> Result<OriginalDatum<CapturedText>, CatalogError> {
        self.check_inventory(inventory)?;
        self.header(
            RecordHeaderField::SourceType,
            self.source_type.clone(),
            SourceUseRole::Selector,
        )
    }

    pub(super) fn record(&self, inventory: &SourceUses) -> Result<CapturedRecord, CatalogError> {
        let subject = self.subject(inventory)?;
        Ok(CapturedRecord {
            record_index: subject.record_index,
            guid: subject.guid,
            record_path: subject.record_path,
            source_type: self.header(
                RecordHeaderField::SourceType,
                self.source_type.clone(),
                SourceUseRole::Selector,
            )?,
        })
    }

    fn check_inventory(&self, inventory: &SourceUses) -> Result<(), ExportAccountingError> {
        if !self.inventory.is_same_inventory(inventory) {
            return Err(ExportAccountingError::UnboundHealthSubject);
        }
        Ok(())
    }

    fn header<T>(
        &self,
        field: RecordHeaderField,
        value: SourceValue<T>,
        role: SourceUseRole,
    ) -> Result<OriginalDatum<T>, CatalogError> {
        let complete = !matches!(value, SourceValue::Unreadable);
        let role = if complete {
            role
        } else {
            SourceUseRole::Attempted
        };
        let inputs = vec![
            self.frame
                .scope
                .at(field.location(self.key.index()), role)?,
        ];
        Ok(OriginalDatum::observed(value, inputs, complete))
    }
}

fn text_value(
    value: &Result<&str, p4kx_extraction::DataForgeError>,
    texts: &super::HealthTextCapture,
) -> Result<SourceValue<CapturedText>, CatalogError> {
    value.as_ref().map_or_else(
        |_| Ok(SourceValue::Unreadable),
        |value| texts.retain_text(value).map(SourceValue::Present),
    )
}
#[cfg(test)]
#[path = "health_header_receipt_tests.rs"]
mod tests;
