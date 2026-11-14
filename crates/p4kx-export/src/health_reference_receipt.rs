// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceScope, SourceUseRole, SourceUses, SourceValue};

use super::{OriginalDatum, OriginalRecordScope, RecordSubject};
use crate::build::original_input::OriginalCatalogInput;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

/// The original read frame shared by one Record's retained Reference receipts.
#[derive(Debug, Clone)]
pub(super) struct ReferenceFrame {
    scope: SourceScope,
    inventory: SourceUses,
    subject: RecordSubject,
    record_index: u32,
}

impl ReferenceFrame {
    pub(super) fn read(
        input: &OriginalCatalogInput<'_, '_>,
        admission: &RecordObservation<'_, '_>,
        scope: &OriginalRecordScope<'_, '_>,
    ) -> Result<Self, CatalogError> {
        admission.subject().check_input(input)?;
        admission.check_tree(scope.tree())?;
        input.record_context(scope.record())?;
        if !scope.sources().is_same_inventory(&input.origins().uses)
            || scope.record().index() != admission.record().index()
            || scope.dataforge().source_ref() != input.origins().dataforge.source_ref()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Self {
            scope: scope.dataforge().clone(),
            inventory: scope.sources().clone(),
            subject: admission.subject().clone(),
            record_index: scope.record().index(),
        })
    }

    pub(super) fn check_scope(
        &self,
        scope: &OriginalRecordScope<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        if !self.inventory.is_same_inventory(scope.sources())
            || self.scope.source_ref() != scope.dataforge().source_ref()
            || self.record_index != scope.record().index()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(())
    }

    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        self.subject.check_input(input)?;
        if !self.inventory.is_same_inventory(&input.origins().uses)
            || self.scope.source_ref() != input.origins().dataforge.source_ref()
        {
            return Err(ExportAccountingError::UnboundHealthSubject);
        }
        Ok(())
    }

    pub(super) fn at(
        &self,
        location: SourceLocation,
        role: SourceUseRole,
    ) -> Result<p4kx_extraction::SourceUse, p4kx_extraction::SourceEvidenceError> {
        self.scope.at(location, role)
    }

    pub(super) const fn subject(&self) -> &RecordSubject {
        &self.subject
    }

    pub(super) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if !self.inventory.is_same_inventory(&other.inventory)
            || self.scope.source_ref() != other.scope.source_ref()
            || self.record_index != other.record_index
        {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.subject.merge(other.subject)
    }
}

/// One actual input coordinate and role retained before a public datum is selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReferenceUse {
    location: SourceLocation,
    role: SourceUseRole,
}

impl ReferenceUse {
    pub(super) const fn captured(location: SourceLocation, role: SourceUseRole) -> Self {
        Self { location, role }
    }
}

/// A private Reference datum whose contributions are materialized only on retention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReferenceDatum<T> {
    value: SourceValue<T>,
    inputs: Vec<ReferenceUse>,
    complete: bool,
}

impl<T> ReferenceDatum<T> {
    pub(super) const fn captured(value: SourceValue<T>, inputs: Vec<ReferenceUse>) -> Self {
        Self {
            value,
            inputs,
            complete: true,
        }
    }

    pub(super) const fn absent() -> Self {
        Self {
            value: SourceValue::Absent,
            inputs: Vec::new(),
            complete: true,
        }
    }

    pub(super) const fn unproved(value: T) -> Self {
        Self {
            value: SourceValue::Present(value),
            inputs: Vec::new(),
            complete: false,
        }
    }

    pub(super) const fn value(&self) -> &SourceValue<T> {
        &self.value
    }

    pub(super) fn contribute(&mut self, input: ReferenceUse) {
        self.inputs.push(input);
    }

    pub(super) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError>
    where
        T: PartialEq,
    {
        if self.value != other.value || self.complete != other.complete {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.inputs.extend(other.inputs);
        Ok(())
    }

    pub(super) fn finish(self, frame: &ReferenceFrame) -> Result<OriginalDatum<T>, CatalogError> {
        let inputs = self
            .inputs
            .into_iter()
            .map(|input| frame.scope.at(input.location, input.role))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(OriginalDatum::observed(self.value, inputs, self.complete))
    }
}

#[cfg(test)]
#[path = "health_reference_receipt_tests.rs"]
mod tests;
