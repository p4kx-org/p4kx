// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::HealthText;
use crate::catalog_number_health::NumberRequirement;
use crate::catalog_read::original::RecordSubject;
use crate::catalog_read::original::ReferenceObservation;
use crate::catalog_reference::CatalogReference;
use crate::export_diagnostic::ExportSourceRole;
use crate::health_diagnostic_observation::DiagnosticObservation as ExportDiagnostic;
use crate::selection::Kind;

/// One inspection of every materialized canonical value, independently of its emitted rows.
#[derive(Debug, Clone)]
#[must_use]
pub(crate) struct FieldInspection {
    pub(crate) kind: Option<Kind>,
    pub(crate) record_index: u32,
    pub(crate) guid: Guid,
    pub(crate) record_path: HealthText,
    pub(crate) subject: RecordSubject,
    pub(crate) role: ExportSourceRole,
    pub(crate) complete: bool,
    pub(crate) diagnostics: Vec<ExportDiagnostic>,
    /// Conditional failures until the exact physical source has an emitted lossless row.
    pub(crate) number_requirements: Vec<NumberRequirement>,
    pub(crate) unresolved_references: Vec<ReferenceObservation>,
    pub(crate) catalog_references: Vec<CatalogReference>,
}

impl FieldInspection {
    /// Reuses the same immutable canonical inspection for an explicit source role or output kind.
    pub(crate) fn contextualize(&mut self, kind: Option<Kind>, role: ExportSourceRole) {
        self.kind = kind;
        self.role = role;
        for diagnostic in &mut self.diagnostics {
            diagnostic.kind = kind;
            diagnostic.source_role = Some(role);
        }
        for reference in &mut self.unresolved_references {
            reference.contextualize(kind, role);
        }
        for reference in &mut self.catalog_references {
            reference.original.contextualize(kind, role);
        }
    }
}

impl FieldInspection {
    pub(crate) fn merge(&mut self, other: Self) -> Result<(), crate::ExportAccountingError> {
        if self != &other {
            return Err(crate::ExportAccountingError::ConflictingFieldInspection);
        }
        self.subject.merge(other.subject)?;
        for (required, other) in self
            .number_requirements
            .iter_mut()
            .zip(other.number_requirements)
        {
            required.merge(other)?;
        }
        for (reference, other) in self
            .unresolved_references
            .iter()
            .zip(other.unresolved_references)
        {
            reference.merge(other)?;
        }
        for (reference, other) in self.catalog_references.iter().zip(other.catalog_references) {
            reference.original.merge(other.original)?;
        }
        for diagnostic in &mut self.diagnostics {
            for example in &mut diagnostic.examples {
                example.merge_owner(&self.subject)?;
            }
        }
        Ok(())
    }
}

// Canonical value equality is checked before the separate original-subject merge.
impl PartialEq for FieldInspection {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.record_index == other.record_index
            && self.guid == other.guid
            && self.record_path == other.record_path
            && self.role == other.role
            && self.complete == other.complete
            && self.diagnostics == other.diagnostics
            && self.number_requirements == other.number_requirements
            && self.unresolved_references == other.unresolved_references
            && self.catalog_references == other.catalog_references
    }
}

impl Eq for FieldInspection {}
