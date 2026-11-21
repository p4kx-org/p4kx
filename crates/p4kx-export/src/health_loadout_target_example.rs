// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::health_example_description::ExampleDescription;
use super::health_loadout_target::LoadoutTargetReceipt;
use super::{OriginalDatum, RecordSubject};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError};
use p4kx_extraction::{Guid, SourceValue};

/// An actual selected loadout target and the diagnostic owner's original Header type.
#[derive(Debug, Clone)]
pub(super) struct LoadoutTargetExample {
    subject: RecordSubject,
    guid: Guid,
    targets: Vec<LoadoutTargetReceipt>,
}

impl LoadoutTargetExample {
    pub(super) fn read(
        input: &OriginalCatalogInput<'_, '_>,
        subject: RecordSubject,
        target: LoadoutTargetReceipt,
    ) -> Result<Self, CatalogError> {
        subject.check_input(input)?;
        target.check_input(input)?;
        subject.source_type_text()?;
        let guid = target.value()?;
        Ok(Self {
            subject,
            guid,
            targets: vec![target],
        })
    }

    pub(super) fn description(&self) -> Result<ExampleDescription, ExportAccountingError> {
        Ok((None, self.subject.source_type_text()?, Some(self.guid)))
    }

    pub(super) fn matches_description(
        &self,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
    ) -> bool {
        field.is_none()
            && self
                .subject
                .source_type_text()
                .is_ok_and(|original| original.as_deref() == source_type)
            && Some(self.guid) == target_guid
    }

    pub(super) const fn subject(&self) -> &RecordSubject {
        &self.subject
    }

    pub(super) fn retain_target(&self) -> Result<OriginalDatum<Guid>, CatalogError> {
        let mut inputs = Vec::new();
        let mut complete = true;
        for target in &self.targets {
            let original = target.retain()?;
            let (uses, known) = original.contributions();
            inputs.extend_from_slice(uses);
            complete &= known;
        }
        Ok(OriginalDatum::observed(
            SourceValue::Present(self.guid),
            inputs,
            complete,
        ))
    }

    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        self.subject.check_input(input)?;
        for target in &self.targets {
            target.check_input(input)?;
        }
        Ok(())
    }

    pub(super) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if self.guid != other.guid {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.subject.merge(other.subject)?;
        self.targets.extend(other.targets);
        Ok(())
    }
}
