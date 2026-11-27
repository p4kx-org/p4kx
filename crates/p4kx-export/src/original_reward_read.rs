// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, Guid, RecordTree, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceValue, SourcedValue, Value,
};

use super::{
    OriginalDatum, OriginalReferenceObservation, OriginalValueEvent, OriginalValuePart,
    RecordSubject,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::record_header_field::RecordHeaderField;
use crate::record_observation::RecordObservation;
use crate::resource_integer::ResourceInt64;
use crate::{CatalogError, ExportAccountingError};

/// The actual read of a selected reward dependency, without an eager payload claim.
pub(crate) struct RewardRead {
    subject: RecordSubject,
    value: SourceValue<i64>,
    origin: (SourceScope, SourceLocation, SourceUseRole),
}

impl RewardRead {
    pub(crate) fn from_tree<'source>(
        input: &OriginalCatalogInput<'_, '_>,
        admission: &RecordObservation<'_, 'source>,
        tree: &RecordTree<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let scope = input.record_scope(admission.record(), tree)?;
        admission.subject().check_input(input)?;
        let Some(original) = tree.field("reputationAmount") else {
            return Ok(Self {
                subject: admission.subject().clone(),
                value: SourceValue::Absent,
                origin: (
                    scope.dataforge().clone(),
                    scope.root_declaration_location(),
                    SourceUseRole::Selector,
                ),
            });
        };
        let mut walk = scope.walk(cancel);
        while let Some(event) = walk.next_event()? {
            let OriginalValueEvent::Value(site) = event else {
                continue;
            };
            if site.parent_ordinal().is_some() || !std::ptr::eq(site.value(), original) {
                continue;
            }
            let fact = site.fact();
            let value = match fact.value() {
                Value::Null => SourceValue::Null,
                Value::Error(_) => SourceValue::Unreadable,
                value => crate::record_fields::integer_value(value)
                    .map_or(SourceValue::Invalid, SourceValue::Present),
            };
            let role = match value {
                SourceValue::Present(_) => SourceUseRole::Payload,
                SourceValue::Unreadable => SourceUseRole::Attempted,
                _ => SourceUseRole::Selector,
            };
            return Ok(Self {
                subject: admission.subject().clone(),
                value,
                origin: (
                    scope.dataforge().clone(),
                    fact.location(OriginalValuePart::Value),
                    role,
                ),
            });
        }
        Err(ExportAccountingError::ConflictingSourceDatum.into())
    }

    pub(crate) fn failed(
        input: &OriginalCatalogInput<'_, '_>,
        admission: &RecordObservation<'_, '_>,
        failure: &crate::CatalogReadFailure<'_>,
    ) -> Result<Self, CatalogError> {
        admission.subject().check_input(input)?;
        let context = input.record_context(admission.record())?;
        input.record_context(failure.record())?;
        if failure.canonical_error().is_none()
            || failure.record().index() != admission.record().index()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Self {
            subject: admission.subject().clone(),
            value: SourceValue::Unreadable,
            origin: (
                context.dataforge().clone(),
                SourceLocation::DataForge {
                    record_index: admission.record().index(),
                    field: String::new(),
                },
                SourceUseRole::Attempted,
            ),
        })
    }

    /// Retains the actual failed path classification without claiming a Reward body read.
    pub(crate) fn unreadable_path(
        input: &OriginalCatalogInput<'_, '_>,
        admission: &RecordObservation<'_, '_>,
    ) -> Result<Self, CatalogError> {
        admission.subject().check_input(input)?;
        let record = admission.record();
        let context = input.record_context(record)?;
        Ok(Self {
            subject: admission.subject().clone(),
            value: SourceValue::Unreadable,
            origin: (
                context.dataforge().clone(),
                RecordHeaderField::FileName.location(record.index()),
                SourceUseRole::Attempted,
            ),
        })
    }

    #[cfg(test)]
    pub(crate) const fn amount(&self) -> Option<i64> {
        match self.value {
            SourceValue::Present(amount) => Some(amount),
            _ => None,
        }
    }

    pub(crate) fn retain(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        reference: &OriginalReferenceObservation<'_, '_, '_, '_>,
    ) -> Result<SourcedValue<ResourceInt64>, CatalogError> {
        self.subject.check_input(input)?;
        reference.caller().check_input(input)?;
        if reference.is_instance() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let record = reference.record()?;
        input.record_context(&record)?;
        let key = self
            .subject
            .key()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        if key.index() != record.index() || key.guid() != record.guid() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let selector = reference
            .caller()
            .observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
        let mut uses = vec![selector];
        self.retain_inputs(&mut uses)?;
        let complete = !matches!(self.value, SourceValue::Unreadable);
        let value =
            OriginalDatum::observed(self.value.clone(), uses, complete).map(ResourceInt64::new)?;
        Ok(value.into_cell(&input.origins().uses))
    }

    /// Retains the exact dependency selected by a nominal Reward lookup.
    pub(crate) fn retain_nominal(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        key: (Guid, u32),
        uses: &mut Vec<SourceUse>,
    ) -> Result<SourceValue<i64>, CatalogError> {
        self.retain_candidate(
            input,
            key,
            uses,
            &[
                RecordHeaderField::Guid,
                RecordHeaderField::RecordIndex,
                RecordHeaderField::FileName,
            ],
        )
    }

    pub(crate) fn retain_unreadable_path(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        key: (Guid, u32),
        uses: &mut Vec<SourceUse>,
    ) -> Result<SourceValue<i64>, CatalogError> {
        self.retain_candidate(
            input,
            key,
            uses,
            &[RecordHeaderField::Guid, RecordHeaderField::RecordIndex],
        )
    }

    fn retain_candidate(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        (guid, record_index): (Guid, u32),
        uses: &mut Vec<SourceUse>,
        selectors: &[RecordHeaderField],
    ) -> Result<SourceValue<i64>, CatalogError> {
        self.subject.check_input(input)?;
        let key = self
            .subject
            .key()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        if key.guid() != guid || key.index() != record_index {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        uses.try_reserve(selectors.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for field in selectors {
            uses.push(
                self.origin
                    .0
                    .at(field.location(record_index), SourceUseRole::Selector)?,
            );
        }
        self.retain_inputs(uses)?;
        Ok(self.value.clone())
    }

    fn retain_inputs(&self, uses: &mut Vec<SourceUse>) -> Result<(), CatalogError> {
        let (scope, location, role) = &self.origin;
        uses.try_reserve(1 + usize::from(*role == SourceUseRole::Payload))
            .map_err(CatalogError::SourceUseAllocation)?;
        uses.push(scope.at(location.clone(), *role)?);
        if *role == SourceUseRole::Payload {
            uses.push(scope.at(location.clone(), SourceUseRole::Conversion)?);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "original_reward_read_tests.rs"]
mod tests;
