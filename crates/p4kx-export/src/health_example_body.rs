// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{FieldOrigin, Guid, SourceUseRef, SourceValue};

use super::health_example_description::ExampleDescription;
use super::health_example_header::HeaderExampleRecipe;
use super::health_reference_observation::{ReferenceEvidence, ReferenceObservation};
use super::health_reference_receipt::{ReferenceDatum, ReferenceFrame};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError, HealthText};

#[path = "health_example_body_merge.rs"]
mod health_example_body_merge;
#[path = "health_field_example_recipe.rs"]
mod health_field_example_recipe;
#[path = "health_example_technical.rs"]
mod technical;

use self::health_field_example_recipe::FieldExampleRecipe;

/// Deferred body facts captured at the actual source-read operation.
#[derive(Debug, Clone)]
pub(crate) struct ExampleBodyReceipt {
    body: StoredExampleBody,
}

#[derive(Debug, Clone)]
enum StoredExampleBody {
    Field(FieldExampleRecipe),
    Full(Arc<ExampleBody>),
}

#[derive(Debug, Clone)]
pub(super) enum ExampleBody {
    Header(HeaderExampleRecipe),
    Captured {
        frame: Arc<ReferenceFrame>,
        field: ReferenceDatum<HealthText>,
        source_type: ReferenceDatum<HealthText>,
        target_guid: ReferenceDatum<Guid>,
    },
    Technical {
        subject: super::RecordSubject,
        field: HealthText,
        evidence: Option<DiagnosticFieldEvidence>,
    },
    Loadout(super::health_loadout_body::LoadoutBodyReceipt),
    RequestedLoadout(super::health_loadout_example::LoadoutRequestExample),
    LoadoutTarget(super::health_loadout_target_example::LoadoutTargetExample),
    Absent(super::RecordSubject),
    GuidHeader(super::RecordSubject),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DiagnosticFieldEvidence {
    pub(super) origin: FieldOrigin,
    pub(super) failed_source_uses: Box<[SourceUseRef]>,
}

impl ExampleBodyReceipt {
    fn full(body: ExampleBody) -> Self {
        Self {
            body: StoredExampleBody::Full(Arc::new(body)),
        }
    }

    pub(super) const fn field_path(
        frame: Arc<ReferenceFrame>,
        path: super::health_reference_path::ReferencePathReceipt,
    ) -> Self {
        Self {
            body: StoredExampleBody::Field(FieldExampleRecipe::new(frame, path)),
        }
    }

    pub(super) fn captured(
        frame: Arc<ReferenceFrame>,
        field: ReferenceDatum<HealthText>,
        source_type: ReferenceDatum<HealthText>,
        target_guid: ReferenceDatum<Guid>,
    ) -> Self {
        Self::full(ExampleBody::Captured {
            frame,
            field,
            source_type,
            target_guid,
        })
    }

    pub(crate) fn source_type_header(
        input: &OriginalCatalogInput<'_, '_>,
        subject: super::RecordSubject,
    ) -> Result<Self, CatalogError> {
        Ok(Self::full(ExampleBody::Header(
            HeaderExampleRecipe::source_type(input, subject)?,
        )))
    }

    pub(crate) fn duplicate_guid_header(
        input: &OriginalCatalogInput<'_, '_>,
        subject: super::RecordSubject,
    ) -> Result<Self, CatalogError> {
        Ok(Self::full(ExampleBody::Header(
            HeaderExampleRecipe::duplicate_guid(input, subject)?,
        )))
    }
    pub(crate) fn loadout(receipt: super::health_loadout_body::LoadoutBodyReceipt) -> Self {
        Self::full(ExampleBody::Loadout(receipt))
    }

    pub(crate) fn requested_loadout(
        input: &OriginalCatalogInput<'_, '_>,
        subject: super::RecordSubject,
        path: super::AuthoredEntryPath,
        selectors: Vec<super::AuthoredEntryPath>,
    ) -> Result<Self, CatalogError> {
        let receipt = super::health_loadout_example::LoadoutRequestExample::read(
            input, subject, path, selectors,
        )?;
        Ok(Self::full(ExampleBody::RequestedLoadout(receipt)))
    }

    pub(crate) fn loadout_target(
        input: &OriginalCatalogInput<'_, '_>,
        subject: super::RecordSubject,
        target: super::LoadoutTargetReceipt,
    ) -> Result<Self, CatalogError> {
        let receipt = super::health_loadout_target_example::LoadoutTargetExample::read(
            input, subject, target,
        )?;
        Ok(Self::full(ExampleBody::LoadoutTarget(receipt)))
    }

    pub(crate) fn absent(subject: super::RecordSubject) -> Self {
        Self::full(ExampleBody::Absent(subject))
    }

    pub(crate) fn guid_header(subject: super::RecordSubject) -> Self {
        Self::full(ExampleBody::GuidHeader(subject))
    }

    pub(crate) fn reference(
        original: &ReferenceObservation,
    ) -> Result<Self, ExportAccountingError> {
        let ReferenceEvidence::Captured { frame, body } = &original.evidence else {
            return Err(ExportAccountingError::UnboundHealthSubject);
        };
        Ok(Self::captured(
            Arc::clone(frame),
            body.field.clone(),
            body.source_type.clone(),
            body.target_guid.clone(),
        ))
    }

    pub(super) fn into_body(self) -> ExampleBody {
        match self.body {
            StoredExampleBody::Field(recipe) => recipe.into_body(),
            StoredExampleBody::Full(body) => Arc::unwrap_or_clone(body),
        }
    }

    pub(crate) fn description(&self) -> Result<ExampleDescription, ExportAccountingError> {
        match &self.body {
            StoredExampleBody::Field(recipe) => Ok(recipe.description()),
            StoredExampleBody::Full(body) => body.description(),
        }
    }

    pub(crate) fn select_description(
        self,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
    ) -> Option<Self> {
        if matches!(&self.body, StoredExampleBody::Field(recipe)
            if recipe.matches(field, source_type, target_guid))
        {
            return Some(self);
        }
        let mut body = self.into_body();
        body.select_fields(field, source_type, target_guid);
        body.matches_description(field, source_type, target_guid)
            .then(|| Self::full(body))
    }

    pub(crate) fn check_description(
        &self,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
    ) -> Result<(), ExportAccountingError> {
        let matches = match &self.body {
            StoredExampleBody::Field(recipe) => recipe.matches(field, source_type, target_guid),
            StoredExampleBody::Full(body) => {
                body.matches_description(field, source_type, target_guid)
            }
        };
        if !matches {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(())
    }

    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        match &self.body {
            StoredExampleBody::Field(recipe) => recipe.frame().check_input(input),
            StoredExampleBody::Full(body) => body.check_input(input),
        }
    }

    pub(crate) fn check_record_owner(
        &self,
        subject: &super::RecordSubject,
    ) -> Result<(), ExportAccountingError> {
        let frame = match &self.body {
            StoredExampleBody::Field(recipe) => recipe.frame(),
            StoredExampleBody::Full(body) => {
                let ExampleBody::Captured { frame, .. } = body.as_ref() else {
                    return Err(ExportAccountingError::UnboundHealthSubject);
                };
                frame
            }
        };
        let mut original = frame.subject().clone();
        original.merge(subject.clone())
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        let other = other.into_body();
        if let StoredExampleBody::Full(body) = &mut self.body {
            return Arc::make_mut(body).merge(other);
        }
        let mut body = self.clone().into_body();
        body.merge(other)?;
        *self = Self::full(body);
        Ok(())
    }
}

impl ExampleBody {
    fn description(&self) -> Result<ExampleDescription, ExportAccountingError> {
        match self {
            Self::Header(recipe) => recipe.description(),
            Self::Captured {
                field,
                source_type,
                target_guid,
                ..
            } => Ok((
                described(field.value())?,
                described(source_type.value())?,
                described(target_guid.value())?,
            )),
            Self::Technical { subject, field, .. } => {
                Ok((Some(field.clone()), subject.source_type_text()?, None))
            }
            Self::Loadout(receipt) => Ok((Some(receipt.path().clone()), None, None)),
            Self::RequestedLoadout(receipt) => Ok((
                Some(receipt.field().clone()),
                receipt.subject().source_type_text()?,
                None,
            )),
            Self::LoadoutTarget(receipt) => receipt.description(),
            Self::Absent(_) => Ok((None, None, None)),
            Self::GuidHeader(subject) => {
                Ok((None, None, subject.key().map(super::RecordSubjectKey::guid)))
            }
        }
    }

    fn select_fields(
        &mut self,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
    ) {
        if let Self::Captured {
            field: actual_field,
            source_type: actual_type,
            target_guid: actual_guid,
            ..
        } = self
        {
            if field.is_none() {
                *actual_field = ReferenceDatum::absent();
            }
            if source_type.is_none() {
                *actual_type = ReferenceDatum::absent();
            }
            if target_guid.is_none() {
                *actual_guid = ReferenceDatum::absent();
            }
        }
    }

    fn matches_description(
        &self,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
    ) -> bool {
        match self {
            Self::Header(recipe) => recipe.matches_description(field, source_type, target_guid),
            Self::Captured {
                field: actual_field,
                source_type: actual_type,
                target_guid: actual_guid,
                ..
            } => {
                text_matches(actual_field.value(), field)
                    && text_matches(actual_type.value(), source_type)
                    && guid_matches(actual_guid.value(), target_guid)
            }
            Self::Technical {
                subject,
                field: original,
                ..
            } => {
                field == Some(original.as_str())
                    && subject
                        .source_type_text()
                        .is_ok_and(|value| value.as_deref() == source_type)
                    && target_guid.is_none()
            }
            Self::Loadout(receipt) => {
                field == Some(receipt.path().as_str())
                    && source_type.is_none()
                    && target_guid.is_none()
            }
            Self::RequestedLoadout(receipt) => {
                field == Some(receipt.field().as_str())
                    && receipt
                        .subject()
                        .source_type_text()
                        .is_ok_and(|value| value.as_deref() == source_type)
                    && target_guid.is_none()
            }
            Self::LoadoutTarget(receipt) => {
                receipt.matches_description(field, source_type, target_guid)
            }
            Self::Absent(_) => field.is_none() && source_type.is_none() && target_guid.is_none(),
            Self::GuidHeader(subject) => {
                field.is_none()
                    && source_type.is_none()
                    && target_guid == subject.key().map(super::RecordSubjectKey::guid)
            }
        }
    }

    fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        match self {
            Self::Header(recipe) => recipe.check_input(input),
            Self::Captured { frame, .. } => frame.check_input(input),
            Self::Loadout(receipt) => receipt.check_input(input),
            Self::RequestedLoadout(receipt) => receipt.check_input(input),
            Self::LoadoutTarget(receipt) => receipt.check_input(input),
            Self::Absent(subject) | Self::GuidHeader(subject) | Self::Technical { subject, .. } => {
                subject.check_input(input)
            }
        }
    }
}

fn text_matches(value: &SourceValue<HealthText>, expected: Option<&str>) -> bool {
    match (value, expected) {
        (SourceValue::Present(value), Some(expected)) => value.as_str() == expected,
        (SourceValue::Absent, None) => true,
        _ => false,
    }
}

fn guid_matches(value: &SourceValue<Guid>, expected: Option<Guid>) -> bool {
    match (value, expected) {
        (SourceValue::Present(value), Some(expected)) => *value == expected,
        (SourceValue::Absent, None) => true,
        _ => false,
    }
}

fn described<T: Clone>(value: &SourceValue<T>) -> Result<Option<T>, ExportAccountingError> {
    match value {
        SourceValue::Present(value) => Ok(Some(value.clone())),
        SourceValue::Absent => Ok(None),
        _ => Err(ExportAccountingError::ConflictingSourceDatum),
    }
}
