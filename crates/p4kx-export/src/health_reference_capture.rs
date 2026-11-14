// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::SourceValue;

use super::health_reference_data::{ReferenceData, ReferencePointer};
use super::health_reference_error;
use super::health_reference_observation::{ReferenceBody, ReferenceObservation};
use super::health_reference_path::ReferencePath;
use super::health_reference_receipt::{ReferenceDatum, ReferenceFrame};
use super::health_reference_target;
use super::health_text_capture::HealthTextCapture;
use super::{OriginalRecordScope, OriginalValueSite};
use crate::build::original_input::OriginalCatalogInput;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, ExportReferenceStatus};

/// Actual original Reference admissions and path receipts for one canonical read.
pub(crate) struct ReferenceCapture<'scope, 'read, 'source> {
    original_scope: &'scope OriginalRecordScope<'read, 'source>,
    pub(super) frame: Arc<ReferenceFrame>,
    pub(super) texts: &'scope HealthTextCapture,
    path: ReferencePath,
}

impl<'scope, 'read, 'source> ReferenceCapture<'scope, 'read, 'source> {
    pub(crate) fn new(
        input: &'scope OriginalCatalogInput<'_, '_>,
        admission: &RecordObservation<'_, '_>,
        scope: &'scope OriginalRecordScope<'read, 'source>,
    ) -> Result<Self, CatalogError> {
        let frame = Arc::new(ReferenceFrame::read(input, admission, scope)?);
        let texts = input.origins().health_texts();
        Ok(Self {
            original_scope: scope,
            frame,
            texts,
            path: ReferencePath::new(scope, texts)?,
        })
    }

    pub(super) fn check_site(
        &self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
    ) -> Result<(), ExportAccountingError> {
        if !std::ptr::eq(self.original_scope, site.fact().scope()) {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.frame.check_scope(site.fact().scope())?;
        self.path.check_site(site)
    }

    pub(crate) fn enter(
        &mut self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
    ) -> Result<(), CatalogError> {
        self.check_site(site)?;
        self.path.enter(site, self.texts)
    }

    pub(crate) fn leave(&mut self, ordinal: u64) -> Result<(), ExportAccountingError> {
        self.path.leave(ordinal)
    }

    pub(crate) fn reference(
        &self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
    ) -> Result<ReferenceObservation, CatalogError> {
        self.check_site(site)?;
        health_reference_target::read(self, site)
    }

    pub(crate) fn error(
        &self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
    ) -> Result<Option<ReferenceObservation>, CatalogError> {
        self.check_site(site)?;
        health_reference_error::read(self, site)
    }

    pub(crate) fn example(
        &self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
    ) -> Result<super::health_example_body::ExampleBodyReceipt, CatalogError> {
        self.check_site(site)?;
        let body = self.body(site)?;
        Ok(super::health_example_body::ExampleBodyReceipt::captured(
            Arc::clone(&self.frame),
            body.field,
            body.source_type,
            body.target_guid,
        ))
    }

    pub(super) fn body(
        &self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
    ) -> Result<ReferenceBody, CatalogError> {
        self.check_site(site)?;
        Ok(ReferenceBody {
            field: self.path.field(site, self.texts)?,
            source_type: self.path.owner_type(site)?,
            target_guid: ReferenceDatum::absent(),
            target_record_index: ReferenceDatum::absent(),
            target_record_path: ReferenceDatum::absent(),
            target_type: ReferenceDatum::absent(),
            target_location: ReferenceDatum::absent(),
            source_pointer: None,
        })
    }

    pub(super) fn observation(
        &self,
        status: Option<ExportReferenceStatus>,
        body: ReferenceBody,
    ) -> Result<ReferenceObservation, CatalogError> {
        let field = described(&body.field)?.ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let source_type =
            described(&body.source_type)?.ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let pointer = body
            .source_pointer
            .as_ref()
            .map(|pointer| {
                Ok::<_, CatalogError>(ReferencePointer {
                    structure: described(&pointer.structure)?,
                    instance: described(&pointer.instance)?,
                    declared_type: described(&pointer.declared_type)?,
                })
            })
            .transpose()?;
        let data = ReferenceData {
            field,
            source_type,
            target_guid: described(&body.target_guid)?,
            target_record_index: described(&body.target_record_index)?,
            target_record_path: described(&body.target_record_path)?,
            target_type: described(&body.target_type)?,
            target_location: described(&body.target_location)?,
            source_pointer: pointer,
        };
        Ok(ReferenceObservation::captured(
            data,
            status,
            Arc::clone(&self.frame),
            body,
        ))
    }
}

fn described<T: Clone>(fact: &ReferenceDatum<T>) -> Result<Option<T>, ExportAccountingError> {
    match fact.value() {
        SourceValue::Present(value) => Ok(Some(value.clone())),
        SourceValue::Absent => Ok(None),
        _ => Err(ExportAccountingError::ConflictingSourceDatum),
    }
}

#[cfg(test)]
#[path = "health_reference_capture_path_tests.rs"]
mod path_tests;
#[cfg(test)]
#[path = "health_reference_capture_scope_tests.rs"]
mod scope_tests;
