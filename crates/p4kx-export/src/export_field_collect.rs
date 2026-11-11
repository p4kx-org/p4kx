// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{Guid, RecordTree, Reference, ValueError};

use crate::catalog_number_capture::FloatToken;
use crate::catalog_number_health::NumberRequirement;
use crate::catalog_read::original::{
    ExampleBodyReceipt, RecordSubject, ReferenceData, ReferenceLink, ReferenceObservation,
};
use crate::catalog_reference::CatalogReference;
use crate::catalog_reference_texts::CatalogReferenceTexts;
use crate::export_diagnostic::{
    EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT, ExportDiagnosticCode, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole,
};
use crate::export_field_inspection::FieldInspection;
use crate::health_diagnostic_observation::{
    DiagnosticObservation as ExportDiagnostic, ExampleObservation as ExportDiagnosticExample,
};
use crate::record_observation::RecordObservation;
use crate::resource_raw_policy::ResourceNumberContract;
use crate::selection::Kind;
use crate::{CatalogError, ExportAccountingError, HealthText};

type DiagnosticKey = (
    ExportDiagnosticStage,
    ExportDiagnosticCode,
    ExportDiagnosticSeverity,
    ExportDiagnosticUnit,
);

#[derive(Clone, Copy)]
struct Note {
    stage: ExportDiagnosticStage,
    code: ExportDiagnosticCode,
    severity: ExportDiagnosticSeverity,
    unit: ExportDiagnosticUnit,
    target: Option<Guid>,
}

/// Complete reference inspection with sampled diagnostic examples for one canonical tree.
pub(crate) struct FieldCollector<'t, 'f> {
    tree: &'t RecordTree<'f>,
    record_index: u32,
    record_path: HealthText,
    subject: RecordSubject,
    kind: Option<Kind>,
    role: ExportSourceRole,
    diagnostics: BTreeMap<DiagnosticKey, ExportDiagnostic>,
    unresolved: Vec<ReferenceObservation>,
    catalog_references: Vec<CatalogReference>,
    reference_texts: CatalogReferenceTexts,
    numbers: Vec<NumberRequirement>,
}

impl<'t, 'f> FieldCollector<'t, 'f> {
    pub(crate) fn new(
        tree: &'t RecordTree<'f>,
        admission: &RecordObservation<'_, 'f>,
        kind: Option<Kind>,
        role: ExportSourceRole,
    ) -> Result<Self, CatalogError> {
        admission.check_tree(tree)?;
        let key = admission
            .subject()
            .key()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let record_path = key
            .path_text()
            .cloned()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if record_path.as_str() != tree.record_path() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Self {
            tree,
            record_index: admission.record().index(),
            record_path,
            subject: admission.subject().clone(),
            kind,
            role,
            diagnostics: BTreeMap::new(),
            unresolved: Vec::new(),
            catalog_references: Vec::new(),
            reference_texts: CatalogReferenceTexts::default(),
            numbers: Vec::new(),
        })
    }
    pub(crate) fn note(
        &mut self,
        field: &str,
        source_type: &str,
        code: ExportDiagnosticCode,
        severity: ExportDiagnosticSeverity,
        target: Option<Guid>,
        body: Option<ExampleBodyReceipt>,
    ) -> Result<(), ExportAccountingError> {
        self.at_stage(
            field,
            source_type,
            Note {
                stage: ExportDiagnosticStage::SourceField,
                code,
                severity,
                unit: ExportDiagnosticUnit::Values,
                target,
            },
            body,
        )
    }

    pub(crate) fn non_finite(
        &mut self,
        field: &str,
        source_type: &str,
        original_ordinal: u64,
        contract: Option<ResourceNumberContract>,
        token: FloatToken,
        body: Option<ExampleBodyReceipt>,
    ) {
        self.numbers.push(NumberRequirement {
            record_index: self.record_index,
            original_ordinal,
            field: field.to_owned(),
            source_type: source_type.to_owned(),
            contract,
            token,
            body,
        });
    }

    /// Retains the admitted candidate without expanding its shared descriptive text.
    pub(crate) fn catalog_reference(
        &mut self,
        target_kind: Kind,
        mut original: ReferenceObservation,
    ) {
        original.contextualize(self.kind, self.role);
        self.catalog_references.push(CatalogReference {
            original,
            target_kind,
        });
    }

    pub(crate) fn accounting_reference(
        &mut self,
        field: &str,
        source_type: &str,
        target: &Reference<'_>,
    ) -> ReferenceObservation {
        let data = ReferenceData {
            field: self.reference_texts.retain(field),
            source_type: self.reference_texts.retain(source_type),
            target_guid: Some(target.guid()),
            target_record_index: Some(target.record_index()),
            target_record_path: Some(self.reference_texts.retain(target.record_path())),
            target_type: Some(self.reference_texts.retain(target.type_name())),
            target_location: target
                .location()
                .map(|location| self.reference_texts.retain(location.as_str())),
            source_pointer: None,
        };
        ReferenceObservation::accounting(self.subject.clone(), data, None)
    }

    pub(crate) fn accounting_error(
        &mut self,
        field: &str,
        source_type: &str,
        error: ValueError,
    ) -> Option<ReferenceObservation> {
        let link = ReferenceLink::from_error(error)?;
        let data = ReferenceData {
            field: self.reference_texts.retain(field),
            source_type: self.reference_texts.retain(source_type),
            target_guid: link.target,
            target_record_index: None,
            target_record_path: None,
            target_type: None,
            target_location: None,
            source_pointer: link.pointer,
        };
        Some(ReferenceObservation::accounting(
            self.subject.clone(),
            data,
            Some(link.status),
        ))
    }

    fn at_stage(
        &mut self,
        field: &str,
        source_type: &str,
        note: Note,
        body: Option<ExampleBodyReceipt>,
    ) -> Result<(), ExportAccountingError> {
        let Note {
            stage,
            code,
            severity,
            unit,
            target,
        } = note;
        let diagnostic = self
            .diagnostics
            .entry((stage, code, severity, unit))
            .or_insert_with(|| ExportDiagnostic {
                kind: self.kind,
                stage,
                code,
                severity,
                source_role: Some(self.role),
                warning_code: None,
                count: 0,
                count_unit: unit,
                examples: Vec::new(),
            });
        diagnostic.count += 1;
        if diagnostic.examples.len() == EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT
            && diagnostic
                .examples
                .last()
                .and_then(|example| example.field.as_deref())
                .is_some_and(|last| field > last)
        {
            return Ok(());
        }
        let example = match body {
            Some(body) => ExportDiagnosticExample::captured(
                self.subject.clone(),
                Some(field),
                Some(source_type),
                target,
                body,
            )?,
            None => ExportDiagnosticExample::new(
                self.subject.clone(),
                Some(field),
                Some(source_type),
                target,
            ),
        };
        diagnostic.examples.push(example);
        diagnostic.examples =
            crate::health_example_samples::retain(std::mem::take(&mut diagnostic.examples))?;
        Ok(())
    }

    pub(crate) fn unresolved(&mut self, mut original: ReferenceObservation) {
        original.contextualize(self.kind, self.role);
        self.unresolved.push(original);
    }

    pub(crate) const fn owner_index(&self) -> u32 {
        self.record_index
    }

    pub(crate) fn finish(self) -> FieldInspection {
        let diagnostics = self.diagnostics.into_values().collect();
        let number_requirements = self.numbers;
        FieldInspection {
            record_index: self.record_index,
            kind: self.kind,
            guid: self.tree.guid(),
            record_path: self.record_path,
            subject: self.subject,
            role: self.role,
            complete: true,
            diagnostics,
            number_requirements,
            unresolved_references: self.unresolved,
            catalog_references: self.catalog_references,
        }
    }
}
