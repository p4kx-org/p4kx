// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Guid, SourceValue};

use super::health_example_description::ExampleDescription;
use super::health_example_header::HeaderExampleRecipe;
use super::health_reference_observation::{ReferenceEvidence, ReferenceObservation};
use super::health_reference_receipt::{ReferenceDatum, ReferenceFrame};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError, HealthText};

/// Deferred body facts captured at the actual source-read operation.
#[derive(Debug, Clone)]
pub(crate) struct ExampleBodyReceipt {
    body: ExampleBody,
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
    },
    Loadout(super::health_loadout_body::LoadoutBodyReceipt),
    RequestedLoadout(super::health_loadout_example::LoadoutRequestExample),
    LoadoutTarget(super::health_loadout_target_example::LoadoutTargetExample),
    Absent(super::RecordSubject),
    GuidHeader(super::RecordSubject),
}

impl ExampleBodyReceipt {
    pub(super) const fn captured(
        frame: Arc<ReferenceFrame>,
        field: ReferenceDatum<HealthText>,
        source_type: ReferenceDatum<HealthText>,
        target_guid: ReferenceDatum<Guid>,
    ) -> Self {
        Self {
            body: ExampleBody::Captured {
                frame,
                field,
                source_type,
                target_guid,
            },
        }
    }

    pub(crate) fn source_type_header(
        input: &OriginalCatalogInput<'_, '_>,
        subject: super::RecordSubject,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            body: ExampleBody::Header(HeaderExampleRecipe::source_type(input, subject)?),
        })
    }

    pub(crate) fn duplicate_guid_header(
        input: &OriginalCatalogInput<'_, '_>,
        subject: super::RecordSubject,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            body: ExampleBody::Header(HeaderExampleRecipe::duplicate_guid(input, subject)?),
        })
    }
    pub(crate) fn technical(
        input: &OriginalCatalogInput<'_, '_>,
        subject: super::RecordSubject,
        field: &'static str,
    ) -> Result<Self, CatalogError> {
        subject.check_input(input)?;
        subject.source_type_text()?;
        let field = input
            .origins()
            .health_texts()
            .retain_text(field)?
            .text()
            .clone();
        Ok(Self {
            body: ExampleBody::Technical { subject, field },
        })
    }

    pub(crate) const fn loadout(receipt: super::health_loadout_body::LoadoutBodyReceipt) -> Self {
        Self {
            body: ExampleBody::Loadout(receipt),
        }
    }

    pub(crate) fn requested_loadout(
        input: &OriginalCatalogInput<'_, '_>,
        subject: super::RecordSubject,
        path: super::AuthoredEntryPath,
        selectors: Vec<super::AuthoredEntryPath>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            body: ExampleBody::RequestedLoadout(
                super::health_loadout_example::LoadoutRequestExample::read(
                    input, subject, path, selectors,
                )?,
            ),
        })
    }

    pub(crate) fn loadout_target(
        input: &OriginalCatalogInput<'_, '_>,
        subject: super::RecordSubject,
        target: super::LoadoutTargetReceipt,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            body: ExampleBody::LoadoutTarget(
                super::health_loadout_target_example::LoadoutTargetExample::read(
                    input, subject, target,
                )?,
            ),
        })
    }

    pub(crate) const fn absent(subject: super::RecordSubject) -> Self {
        Self {
            body: ExampleBody::Absent(subject),
        }
    }

    pub(crate) const fn guid_header(subject: super::RecordSubject) -> Self {
        Self {
            body: ExampleBody::GuidHeader(subject),
        }
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
        self.body
    }

    pub(crate) fn description(&self) -> Result<ExampleDescription, ExportAccountingError> {
        match &self.body {
            ExampleBody::Header(recipe) => recipe.description(),
            ExampleBody::Captured {
                field,
                source_type,
                target_guid,
                ..
            } => Ok((
                described(field.value())?,
                described(source_type.value())?,
                described(target_guid.value())?,
            )),
            ExampleBody::Technical { subject, field } => {
                Ok((Some(field.clone()), subject.source_type_text()?, None))
            }
            ExampleBody::Loadout(receipt) => Ok((Some(receipt.path().clone()), None, None)),
            ExampleBody::RequestedLoadout(receipt) => Ok((
                Some(receipt.field().clone()),
                receipt.subject().source_type_text()?,
                None,
            )),
            ExampleBody::LoadoutTarget(receipt) => receipt.description(),
            ExampleBody::Absent(_) => Ok((None, None, None)),
            ExampleBody::GuidHeader(subject) => {
                Ok((None, None, subject.key().map(super::RecordSubjectKey::guid)))
            }
        }
    }

    pub(crate) fn select_description(
        mut self,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
    ) -> Option<Self> {
        if let ExampleBody::Captured {
            field: actual_field,
            source_type: actual_type,
            target_guid: actual_guid,
            ..
        } = &mut self.body
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
        self.matches_description(field, source_type, target_guid)
            .then_some(self)
    }

    fn matches_description(
        &self,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
    ) -> bool {
        match &self.body {
            ExampleBody::Header(recipe) => {
                recipe.matches_description(field, source_type, target_guid)
            }
            ExampleBody::Captured {
                field: actual_field,
                source_type: actual_type,
                target_guid: actual_guid,
                ..
            } => {
                text_matches(actual_field.value(), field)
                    && text_matches(actual_type.value(), source_type)
                    && guid_matches(actual_guid.value(), target_guid)
            }
            ExampleBody::Technical {
                subject,
                field: original,
            } => {
                field == Some(original.as_str())
                    && subject
                        .source_type_text()
                        .is_ok_and(|value| value.as_deref() == source_type)
                    && target_guid.is_none()
            }
            ExampleBody::Loadout(receipt) => {
                field == Some(receipt.path().as_str())
                    && source_type.is_none()
                    && target_guid.is_none()
            }
            ExampleBody::RequestedLoadout(receipt) => {
                field == Some(receipt.field().as_str())
                    && receipt
                        .subject()
                        .source_type_text()
                        .is_ok_and(|value| value.as_deref() == source_type)
                    && target_guid.is_none()
            }
            ExampleBody::LoadoutTarget(receipt) => {
                receipt.matches_description(field, source_type, target_guid)
            }
            ExampleBody::Absent(_) => {
                field.is_none() && source_type.is_none() && target_guid.is_none()
            }
            ExampleBody::GuidHeader(subject) => {
                field.is_none()
                    && source_type.is_none()
                    && target_guid == subject.key().map(super::RecordSubjectKey::guid)
            }
        }
    }

    pub(crate) fn check_description(
        &self,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
    ) -> Result<(), ExportAccountingError> {
        if !self.matches_description(field, source_type, target_guid) {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(())
    }

    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        match &self.body {
            ExampleBody::Header(recipe) => recipe.check_input(input),
            ExampleBody::Captured { frame, .. } => frame.check_input(input),
            ExampleBody::Loadout(receipt) => receipt.check_input(input),
            ExampleBody::RequestedLoadout(receipt) => receipt.check_input(input),
            ExampleBody::LoadoutTarget(receipt) => receipt.check_input(input),
            ExampleBody::Absent(subject)
            | ExampleBody::GuidHeader(subject)
            | ExampleBody::Technical { subject, .. } => subject.check_input(input),
        }
    }

    pub(crate) fn check_record_owner(
        &self,
        subject: &super::RecordSubject,
    ) -> Result<(), ExportAccountingError> {
        let ExampleBody::Captured { frame, .. } = &self.body else {
            return Err(ExportAccountingError::UnboundHealthSubject);
        };
        let mut original = frame.subject().clone();
        original.merge(subject.clone())
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        match (&mut self.body, other.body) {
            (ExampleBody::Header(recipe), ExampleBody::Header(other)) => recipe.merge(other),
            (ExampleBody::Loadout(receipt), ExampleBody::Loadout(other)) => receipt.merge(other),
            (ExampleBody::RequestedLoadout(receipt), ExampleBody::RequestedLoadout(other)) => {
                receipt.merge(other)
            }
            (ExampleBody::LoadoutTarget(receipt), ExampleBody::LoadoutTarget(other)) => {
                receipt.merge(other)
            }
            (
                ExampleBody::Technical { subject, field },
                ExampleBody::Technical {
                    subject: other_subject,
                    field: other_field,
                },
            ) => {
                if *field != other_field {
                    return Err(ExportAccountingError::ConflictingSourceDatum);
                }
                subject.merge(other_subject)
            }
            (ExampleBody::Absent(subject), ExampleBody::Absent(other))
            | (ExampleBody::GuidHeader(subject), ExampleBody::GuidHeader(other)) => {
                subject.merge(other)
            }
            (
                ExampleBody::Captured {
                    frame,
                    field,
                    source_type,
                    target_guid,
                },
                ExampleBody::Captured {
                    frame: other_frame,
                    field: other_field,
                    source_type: other_type,
                    target_guid: other_guid,
                },
            ) => {
                let mut original = frame.as_ref().clone();
                original.merge(other_frame.as_ref().clone())?;
                field.merge(other_field)?;
                source_type.merge(other_type)?;
                target_guid.merge(other_guid)?;
                *frame = Arc::new(original);
                Ok(())
            }
            _ => Err(ExportAccountingError::UnboundHealthSubject),
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
