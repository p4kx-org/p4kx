// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceValue};

use super::health_example_description::ExampleDescription;
use super::health_text_capture::CapturedText;
use super::{OriginalDatum, RecordSubject};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError, HealthText};

#[derive(Debug, Clone, PartialEq, Eq)]
enum HeaderRecipeKind {
    SourceType,
    DuplicateGuid { field: HealthText },
}

/// Original header facts selected by a closed diagnostic operation.
#[derive(Debug, Clone)]
pub(super) struct HeaderExampleRecipe {
    subject: RecordSubject,
    kind: HeaderRecipeKind,
}

pub(super) struct HeaderExampleFacts {
    pub(super) field: OriginalDatum<HealthText>,
    pub(super) source_type: OriginalDatum<CapturedText>,
    pub(super) target_guid: OriginalDatum<Guid>,
}

impl HeaderExampleRecipe {
    pub(super) fn source_type(
        input: &OriginalCatalogInput<'_, '_>,
        subject: RecordSubject,
    ) -> Result<Self, CatalogError> {
        check_subject(input, &subject)?;
        Ok(Self {
            subject,
            kind: HeaderRecipeKind::SourceType,
        })
    }

    pub(super) fn duplicate_guid(
        input: &OriginalCatalogInput<'_, '_>,
        subject: RecordSubject,
    ) -> Result<Self, CatalogError> {
        check_subject(input, &subject)?;
        let field = input
            .origins()
            .health_texts()
            .retain_text("$guid")?
            .text()
            .clone();
        Ok(Self {
            subject,
            kind: HeaderRecipeKind::DuplicateGuid { field },
        })
    }

    pub(super) fn description(&self) -> Result<ExampleDescription, ExportAccountingError> {
        let field = match &self.kind {
            HeaderRecipeKind::SourceType => None,
            HeaderRecipeKind::DuplicateGuid { field } => Some(field.clone()),
        };
        Ok((field, self.subject.source_type_text()?, self.target_guid()?))
    }

    pub(super) fn matches_description(
        &self,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
    ) -> bool {
        self.description()
            .is_ok_and(|(actual_field, actual_type, actual_guid)| {
                actual_field.as_deref() == field
                    && actual_type.as_deref() == source_type
                    && actual_guid == target_guid
            })
    }
    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        check_subject(input, &self.subject)
    }

    pub(super) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if self.kind != other.kind {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.subject.merge(other.subject)
    }

    pub(super) fn retain(
        self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<HeaderExampleFacts, CatalogError> {
        self.check_input(input)?;
        let sources = &input.origins().uses;
        let (field, target_guid) = match self.kind {
            HeaderRecipeKind::SourceType => (
                OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
                OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
            ),
            HeaderRecipeKind::DuplicateGuid { field } => (
                OriginalDatum::observed(SourceValue::Present(field), Vec::new(), true),
                self.subject.retain_guid(sources)?,
            ),
        };
        Ok(HeaderExampleFacts {
            field,
            source_type: self.subject.retain_source_type(sources)?,
            target_guid,
        })
    }

    fn target_guid(&self) -> Result<Option<Guid>, ExportAccountingError> {
        match self.kind {
            HeaderRecipeKind::SourceType => Ok(None),
            HeaderRecipeKind::DuplicateGuid { .. } => Ok(Some(
                self.subject
                    .key()
                    .ok_or(ExportAccountingError::UnboundHealthSubject)?
                    .guid(),
            )),
        }
    }
}

fn check_subject(
    input: &OriginalCatalogInput<'_, '_>,
    subject: &RecordSubject,
) -> Result<(), ExportAccountingError> {
    subject.check_input(input)?;
    subject
        .key()
        .ok_or(ExportAccountingError::UnboundHealthSubject)?;
    Ok(())
}

#[cfg(test)]
#[path = "health_example_header_tests.rs"]
mod tests;
