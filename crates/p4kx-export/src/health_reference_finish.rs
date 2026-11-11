// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUses};

use super::OriginalDatum;
use super::health_diagnostic_subject::DiagnosticSubject;
use super::health_header_receipt::CapturedSubject;
use super::health_reference_cells::{
    ExportSourcePointer, ExportUnresolvedReference, ReferenceBodyCells,
};
use super::health_reference_observation::{ReferenceBody, ReferenceEvidence, ReferenceObservation};
use super::health_reference_receipt::ReferenceFrame;
use super::health_text_capture::{CapturedText, HealthTextCapture, SealedTexts};
use crate::build::original_input::OriginalCatalogInput;
use crate::{
    CatalogError, ExportAccountingError, ExportReferenceStatus, ExportSourceRole, HealthText, Kind,
};

/// One retained unresolved occurrence awaiting the report's single shared text seal.
pub(super) struct RetainedReference {
    kind: Option<Kind>,
    role: ExportSourceRole,
    status: ExportReferenceStatus,
    subject: CapturedSubject,
    body: CapturedBody,
}

struct CapturedBody {
    field: OriginalDatum<CapturedText>,
    source_type: OriginalDatum<CapturedText>,
    target_guid: OriginalDatum<Guid>,
    target_record_index: OriginalDatum<u32>,
    target_record_path: OriginalDatum<CapturedText>,
    target_type: OriginalDatum<CapturedText>,
    target_location: OriginalDatum<CapturedText>,
    source_pointer: Option<CapturedPointer>,
}

struct CapturedPointer {
    structure: OriginalDatum<u32>,
    instance: OriginalDatum<u32>,
    declared_type: OriginalDatum<u32>,
}

impl RetainedReference {
    pub(super) fn read(
        observation: ReferenceObservation,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<Self, CatalogError> {
        let status = observation
            .status()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let ReferenceEvidence::Captured { frame, body } = observation.evidence else {
            return Err(ExportAccountingError::UnboundHealthSubject.into());
        };
        frame.check_input(input)?;
        let subject = frame
            .subject()
            .retain_diagnostic(&input.origins().uses)?
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let body = CapturedBody::read(
            body.as_ref().clone(),
            &frame,
            input.origins().health_texts(),
        )?;
        Ok(Self {
            kind: observation.kind,
            role: observation.source_role,
            status,
            subject,
            body,
        })
    }

    pub(super) const fn texts(&self) -> [&OriginalDatum<CapturedText>; 6] {
        [
            &self.subject.record_path,
            &self.body.field,
            &self.body.source_type,
            &self.body.target_record_path,
            &self.body.target_type,
            &self.body.target_location,
        ]
    }

    pub(super) fn finish(
        self,
        texts: &SealedTexts,
        sources: &SourceUses,
    ) -> Result<ExportUnresolvedReference, ExportAccountingError> {
        let subject = DiagnosticSubject::finish(self.subject, texts, sources)?;
        let body = self.body.finish(texts, sources)?;
        Ok(ExportUnresolvedReference::new(
            self.kind,
            self.role,
            subject,
            body,
            self.status,
        ))
    }
}

impl CapturedBody {
    fn read(
        body: ReferenceBody,
        frame: &ReferenceFrame,
        texts: &HealthTextCapture,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            field: texts.capture_shared(body.field.finish(frame)?)?,
            source_type: texts.capture_shared(body.source_type.finish(frame)?)?,
            target_guid: body.target_guid.finish(frame)?,
            target_record_index: body.target_record_index.finish(frame)?,
            target_record_path: texts.capture_shared(body.target_record_path.finish(frame)?)?,
            target_type: texts.capture_shared(body.target_type.finish(frame)?)?,
            target_location: texts.capture_shared(body.target_location.finish(frame)?)?,
            source_pointer: body
                .source_pointer
                .map(|pointer| {
                    Ok::<_, CatalogError>(CapturedPointer {
                        structure: pointer.structure.finish(frame)?,
                        instance: pointer.instance.finish(frame)?,
                        declared_type: pointer.declared_type.finish(frame)?,
                    })
                })
                .transpose()?,
        })
    }

    fn finish(
        self,
        texts: &SealedTexts,
        sources: &SourceUses,
    ) -> Result<ReferenceBodyCells, ExportAccountingError> {
        Ok(ReferenceBodyCells {
            field: texts.cell(self.field, sources)?,
            source_type: texts.cell(self.source_type, sources)?,
            target_guid: self
                .target_guid
                .map(|guid| HealthText::new(&guid.to_string()))?
                .into_cell(sources),
            target_record_index: self.target_record_index.into_cell(sources),
            target_record_path: texts.cell(self.target_record_path, sources)?,
            target_type: texts.cell(self.target_type, sources)?,
            target_location: texts.cell(self.target_location, sources)?,
            source_pointer: self.source_pointer.map(|pointer| pointer.finish(sources)),
        })
    }
}

impl CapturedPointer {
    fn finish(self, sources: &SourceUses) -> ExportSourcePointer {
        ExportSourcePointer::new(
            self.structure.into_cell(sources),
            self.instance.into_cell(sources),
            self.declared_type.into_cell(sources),
        )
    }
}
