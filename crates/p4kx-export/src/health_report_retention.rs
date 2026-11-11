// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUses;

use super::OriginalDatum;
use super::health_diagnostic_cells::{ExportDiagnostic, ExportDiagnosticExample};
use super::health_diagnostic_subject::DiagnosticSubject;
use super::health_example_retention::RetainedExampleBody;
use super::health_failed_record::FailedExportRecord;
use super::health_header_receipt::CapturedSubject;
use super::health_record_capture::CapturedRecord;
use super::health_record_cells::RecordCells;
use super::health_text_capture::{CapturedText, SealedTexts};
use crate::build::original_input::OriginalCatalogInput;
use crate::health_diagnostic_observation::{DiagnosticObservation, ExampleObservation};
use crate::health_failed_observation::FailedRecordObservation;
use crate::{CatalogError, ExportAccountingError};

pub(super) struct RetainedRecord {
    observation: FailedRecordObservation,
    captured: CapturedRecord,
}
impl RetainedRecord {
    pub(super) fn read(
        observation: FailedRecordObservation,
        sources: &SourceUses,
    ) -> Result<Self, CatalogError> {
        let captured = observation.subject().retain_record(sources)?;
        Ok(Self {
            observation,
            captured,
        })
    }
    pub(super) const fn texts(&self) -> [&OriginalDatum<CapturedText>; 2] {
        [&self.captured.record_path, &self.captured.source_type]
    }
    pub(super) fn finish(
        self,
        texts: &SealedTexts,
        sources: &SourceUses,
    ) -> Result<FailedExportRecord, ExportAccountingError> {
        let header = RecordCells::finish(self.captured, texts, sources)?;
        Ok(FailedExportRecord::new(
            self.observation.kind,
            header,
            self.observation.role,
            self.observation.stage,
            self.observation.code,
        ))
    }
}

struct RetainedExample {
    captured: Option<CapturedSubject>,
    body: RetainedExampleBody,
}
impl RetainedExample {
    fn read(
        observation: &ExampleObservation,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<Self, CatalogError> {
        let receipt = observation.body()?.clone();
        let captured = observation
            .subject()
            .retain_diagnostic(&input.origins().uses)?;
        let body = RetainedExampleBody::read(receipt, input)?;
        Ok(Self { captured, body })
    }
    fn texts(&self) -> impl Iterator<Item = &OriginalDatum<CapturedText>> {
        self.captured
            .as_ref()
            .map(|subject| &subject.record_path)
            .into_iter()
            .chain(self.body.texts())
    }
    fn finish(
        self,
        texts: &SealedTexts,
        sources: &SourceUses,
    ) -> Result<ExportDiagnosticExample, ExportAccountingError> {
        let subject = match self.captured {
            Some(captured) => DiagnosticSubject::finish(captured, texts, sources)?,
            None => DiagnosticSubject::absent(sources),
        };
        Ok(ExportDiagnosticExample::new(
            subject,
            self.body.finish(texts, sources)?,
        ))
    }
}

pub(super) struct RetainedDiagnostic {
    observation: DiagnosticObservation,
    examples: Vec<RetainedExample>,
}
impl RetainedDiagnostic {
    pub(super) fn read(
        mut observation: DiagnosticObservation,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<Self, CatalogError> {
        let examples = std::mem::take(&mut observation.examples)
            .into_iter()
            .map(|example| RetainedExample::read(&example, input))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            observation,
            examples,
        })
    }
    pub(super) fn texts(&self) -> impl Iterator<Item = &OriginalDatum<CapturedText>> {
        self.examples.iter().flat_map(RetainedExample::texts)
    }
    pub(super) fn finish(
        self,
        texts: &SealedTexts,
        sources: &SourceUses,
    ) -> Result<ExportDiagnostic, ExportAccountingError> {
        let examples = self
            .examples
            .into_iter()
            .map(|example| example.finish(texts, sources))
            .collect::<Result<_, _>>()?;
        Ok(ExportDiagnostic {
            kind: self.observation.kind,
            stage: self.observation.stage,
            code: self.observation.code,
            severity: self.observation.severity,
            source_role: self.observation.source_role,
            warning_code: self.observation.warning_code,
            count: self.observation.count,
            count_unit: self.observation.count_unit,
            examples,
        })
    }
}
