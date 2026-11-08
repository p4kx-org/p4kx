// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{Guid, RecordTree};

use crate::catalog_reference::{self, CatalogReference};
use crate::export_diagnostic::{
    EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT, ExportDiagnostic, ExportDiagnosticCode,
    ExportDiagnosticExample, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::export_field_inspection::FieldInspection;
use crate::export_reference_issue::{
    ExportReferenceStatus, ExportSourcePointer, ExportUnresolvedReference,
};
use crate::selection::Kind;

const MAX_CATALOG_REFERENCES: usize = 4096;
const MAX_CATALOG_REFERENCE_METADATA_BYTES: usize = 1024 * 1024;

/// The payload contract that can retain a nonfinite source number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum NonFiniteEncoding {
    FiniteOnly,
    ResourceBits,
}

type DiagnosticKey = (
    ExportDiagnosticStage,
    ExportDiagnosticCode,
    ExportDiagnosticSeverity,
    ExportDiagnosticUnit,
    NonFiniteEncoding,
);

#[derive(Clone, Copy)]
struct Note {
    stage: ExportDiagnosticStage,
    code: ExportDiagnosticCode,
    severity: ExportDiagnosticSeverity,
    unit: ExportDiagnosticUnit,
    target: Option<Guid>,
}

/// Bounded diagnostic aggregation for one canonical tree and its complete unresolved identities.
pub(crate) struct FieldCollector<'t, 'f> {
    tree: &'t RecordTree<'f>,
    record_index: u32,
    kind: Option<Kind>,
    role: ExportSourceRole,
    diagnostics: BTreeMap<DiagnosticKey, ExportDiagnostic>,
    unresolved: Vec<ExportUnresolvedReference>,
    catalog_references: Vec<CatalogReference>,
    catalog_reference_bytes: usize,
    complete: bool,
}

impl<'t, 'f> FieldCollector<'t, 'f> {
    pub(crate) const fn new(
        tree: &'t RecordTree<'f>,
        record_index: u32,
        kind: Option<Kind>,
        role: ExportSourceRole,
    ) -> Self {
        Self {
            tree,
            record_index,
            kind,
            role,
            diagnostics: BTreeMap::new(),
            unresolved: Vec::new(),
            catalog_references: Vec::new(),
            catalog_reference_bytes: 0,
            complete: true,
        }
    }

    pub(crate) fn note(
        &mut self,
        field: &str,
        source_type: &str,
        code: ExportDiagnosticCode,
        severity: ExportDiagnosticSeverity,
        target: Option<Guid>,
    ) {
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
        );
    }

    pub(crate) fn non_finite(
        &mut self,
        field: &str,
        source_type: &str,
        encoding: NonFiniteEncoding,
    ) {
        self.at_stage_encoded(
            field,
            source_type,
            Note {
                stage: ExportDiagnosticStage::Projection,
                code: ExportDiagnosticCode::UnsupportedValue,
                severity: ExportDiagnosticSeverity::Error,
                unit: ExportDiagnosticUnit::Values,
                target: None,
            },
            encoding,
        );
    }

    /// Captures only bounded metadata for an already resolved source Reference.
    pub(crate) fn catalog_reference(
        &mut self,
        field: &str,
        source_type: &str,
        parent_type: Option<&str>,
        reference: &p4kx_extraction::Reference<'_>,
    ) -> bool {
        let Some(target_kind) =
            catalog_reference::target_kind(source_type, parent_type, field, reference)
        else {
            return true;
        };
        let byte_lengths = [
            field.len(),
            source_type.len(),
            reference.record_path().len(),
            reference.type_name().len(),
        ];
        let bytes = byte_lengths
            .into_iter()
            .try_fold(size_of::<CatalogReference>(), usize::checked_add);
        let Some(bytes) = bytes else {
            self.limited(field, source_type);
            return false;
        };
        if self.catalog_references.len() == MAX_CATALOG_REFERENCES
            || bytes
                > MAX_CATALOG_REFERENCE_METADATA_BYTES.saturating_sub(self.catalog_reference_bytes)
        {
            self.limited(field, source_type);
            return false;
        }
        self.catalog_reference_bytes += bytes;
        self.catalog_references.push(CatalogReference {
            field: field.to_owned(),
            source_type: source_type.to_owned(),
            target_record_index: reference.record_index(),
            target_guid: reference.guid(),
            target_record_path: reference.record_path().to_owned(),
            target_type: reference.type_name().to_owned(),
            target_kind,
        });
        true
    }

    pub(crate) fn limited(&mut self, field: &str, source_type: &str) {
        self.complete = false;
        self.at_stage(
            field,
            source_type,
            Note {
                stage: ExportDiagnosticStage::SourceField,
                code: ExportDiagnosticCode::FieldWalkLimit,
                severity: ExportDiagnosticSeverity::Error,
                unit: ExportDiagnosticUnit::Values,
                target: None,
            },
        );
    }

    fn at_stage(&mut self, field: &str, source_type: &str, note: Note) {
        self.at_stage_encoded(field, source_type, note, NonFiniteEncoding::FiniteOnly);
    }

    fn at_stage_encoded(
        &mut self,
        field: &str,
        source_type: &str,
        note: Note,
        encoding: NonFiniteEncoding,
    ) {
        let Note {
            stage,
            code,
            severity,
            unit,
            target,
        } = note;
        let diagnostic = self
            .diagnostics
            .entry((stage, code, severity, unit, encoding))
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
            return;
        }
        diagnostic.examples.push(ExportDiagnosticExample {
            record_index: Some(self.record_index),
            record_guid: Some(self.tree.guid().to_string()),
            record_path: Some(self.tree.record_path().to_owned()),
            field: Some(field.to_owned()),
            source_type: Some(source_type.to_owned()),
            target_guid: target.map(|guid| guid.to_string()),
        });
        diagnostic.examples.sort();
        diagnostic.examples.dedup();
        diagnostic
            .examples
            .truncate(EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT);
    }

    pub(crate) fn unresolved(
        &mut self,
        field: &str,
        source_type: &str,
        target: Option<Guid>,
        status: ExportReferenceStatus,
        pointer: Option<ExportSourcePointer>,
    ) {
        self.unresolved.push(ExportUnresolvedReference {
            record_index: self.record_index,
            kind: self.kind,
            source_role: self.role,
            record_guid: self.tree.guid().to_string(),
            record_path: self.tree.record_path().to_owned(),
            field: field.to_owned(),
            source_type: source_type.to_owned(),
            target_guid: target.map(|guid| guid.to_string()),
            target_record_index: None,
            target_record_path: None,
            target_type: None,
            target_location: None,
            status,
            source_pointer: pointer,
        });
    }

    pub(crate) fn foreign(
        &mut self,
        field: &str,
        source_type: &str,
        target: &p4kx_extraction::Reference<'_>,
    ) {
        self.unresolved.push(ExportUnresolvedReference {
            record_index: self.record_index,
            kind: self.kind,
            source_role: self.role,
            record_guid: self.tree.guid().to_string(),
            record_path: self.tree.record_path().to_owned(),
            field: field.to_owned(),
            source_type: source_type.to_owned(),
            target_guid: Some(target.guid().to_string()),
            target_record_index: Some(target.record_index()),
            target_record_path: Some(target.record_path().to_owned()),
            target_type: Some(target.type_name().to_owned()),
            target_location: target
                .location()
                .map(|location| location.as_str().to_owned()),
            status: ExportReferenceStatus::OutsideOwner,
            source_pointer: None,
        });
    }

    pub(crate) const fn owner_index(&self) -> u32 {
        self.record_index
    }

    pub(crate) fn finish(self) -> FieldInspection {
        let mut diagnostics = Vec::new();
        let mut resource_number_diagnostics = Vec::new();
        for ((_, _, _, _, encoding), diagnostic) in self.diagnostics {
            match encoding {
                NonFiniteEncoding::FiniteOnly => diagnostics.push(diagnostic),
                NonFiniteEncoding::ResourceBits => resource_number_diagnostics.push(diagnostic),
            }
        }
        FieldInspection {
            record_index: self.record_index,
            kind: self.kind,
            guid: self.tree.guid(),
            record_path: self.tree.record_path().to_owned(),
            role: self.role,
            complete: self.complete,
            diagnostics,
            resource_number_diagnostics,
            unresolved_references: self.unresolved,
            catalog_references: self.catalog_references,
        }
    }
}
