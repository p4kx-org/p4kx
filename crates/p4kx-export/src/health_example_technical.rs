// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceUseRef};

use super::super::RecordSubject;
use super::{DiagnosticFieldEvidence, ExampleBody, ExampleBodyReceipt, StoredExampleBody};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;

impl ExampleBodyReceipt {
    pub(crate) fn technical(
        input: &OriginalCatalogInput<'_, '_>,
        subject: RecordSubject,
        field: &'static str,
    ) -> Result<Self, CatalogError> {
        Self::technical_recipe(input, subject, field, None)
    }

    pub(crate) fn technical_with_evidence(
        input: &OriginalCatalogInput<'_, '_>,
        subject: RecordSubject,
        field: &'static str,
        origin: &FieldOrigin,
        failed_source_uses: &[SourceUseRef],
    ) -> Result<Self, CatalogError> {
        let evidence = DiagnosticFieldEvidence {
            origin: origin.clone(),
            failed_source_uses: failed_source_uses.into(),
        };
        Self::technical_recipe(input, subject, field, Some(evidence))
    }

    pub(in super::super) fn field_evidence(&self) -> Option<&DiagnosticFieldEvidence> {
        match &self.body {
            StoredExampleBody::Full(body) => match body.as_ref() {
                ExampleBody::Technical { evidence, .. } => evidence.as_ref(),
                _ => None,
            },
            StoredExampleBody::Field(_) => None,
        }
    }

    fn technical_recipe(
        input: &OriginalCatalogInput<'_, '_>,
        subject: RecordSubject,
        field: &'static str,
        evidence: Option<DiagnosticFieldEvidence>,
    ) -> Result<Self, CatalogError> {
        subject.check_input(input)?;
        subject.source_type_text()?;
        let field = input
            .origins()
            .health_texts()
            .retain_text(field)?
            .text()
            .clone();
        Ok(Self::full(ExampleBody::Technical {
            subject,
            field,
            evidence,
        }))
    }
}
