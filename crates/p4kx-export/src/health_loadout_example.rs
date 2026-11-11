// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;

use super::{AuthoredEntryPath, OriginalDatum, RecordSubject};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError, HealthText};

/// The actual requested file identity and the diagnostic owner's original type.
#[derive(Debug, Clone)]
pub(super) struct LoadoutRequestExample {
    subject: RecordSubject,
    field: HealthText,
    paths: Vec<AuthoredEntryPath>,
    selectors: Vec<AuthoredEntryPath>,
}

impl LoadoutRequestExample {
    pub(super) fn read(
        input: &OriginalCatalogInput<'_, '_>,
        subject: RecordSubject,
        path: AuthoredEntryPath,
        selectors: Vec<AuthoredEntryPath>,
    ) -> Result<Self, CatalogError> {
        subject.check_input(input)?;
        path.check_input(input)?;
        subject.source_type_text()?;
        for selector in &selectors {
            selector.check_input(input)?;
        }
        Ok(Self {
            subject,
            field: path.text().clone(),
            paths: vec![path],
            selectors,
        })
    }

    pub(super) const fn field(&self) -> &HealthText {
        &self.field
    }

    pub(super) const fn subject(&self) -> &RecordSubject {
        &self.subject
    }

    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        self.subject.check_input(input)?;
        for path in &self.paths {
            path.check_input(input)?;
        }
        for selector in &self.selectors {
            selector.check_input(input)?;
        }
        Ok(())
    }

    pub(super) fn retain_field(&self) -> Result<OriginalDatum<HealthText>, CatalogError> {
        let mut inputs = Vec::new();
        let mut complete = true;
        for path in &self.paths {
            let datum = path.retain()?;
            let (original, known) = datum.contributions();
            inputs.extend_from_slice(original);
            complete &= known;
        }
        for selector in &self.selectors {
            let (selected, known) = selector.selector_inputs()?;
            inputs.extend(selected);
            complete &= known;
        }
        Ok(OriginalDatum::observed(
            SourceValue::Present(self.field.clone()),
            inputs,
            complete,
        ))
    }

    pub(super) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if self.field != other.field {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.subject.merge(other.subject)?;
        self.paths.extend(other.paths);
        self.selectors.extend(other.selectors);
        Ok(())
    }
}

#[cfg(test)]
#[path = "health_loadout_example_tests.rs"]
mod tests;
