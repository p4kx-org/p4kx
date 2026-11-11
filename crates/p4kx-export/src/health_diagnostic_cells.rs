// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use super::health_diagnostic_subject::DiagnosticSubject;
use super::health_example_cells::ExampleBodyCells;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::health_metadata::HealthTextRef;
use crate::selection::Kind;
use crate::{HealthText, HealthTextView, schema_fields};

/// One diagnostic example with its directly paired original subject cells.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ExportDiagnosticExample {
    #[serde(flatten)]
    subject: DiagnosticSubject,
    #[serde(flatten)]
    body: ExampleBodyCells,
}

impl ExportDiagnosticExample {
    pub(super) const fn new(subject: DiagnosticSubject, body: ExampleBodyCells) -> Self {
        Self { subject, body }
    }

    /// Borrows the original source-field text cell.
    #[must_use]
    pub const fn field(&self) -> &p4kx_extraction::SourcedValue<HealthTextRef> {
        self.body.field.cell()
    }

    /// Borrows the complete field text with this occurrence's own origin.
    #[must_use]
    pub fn field_text(&self) -> HealthTextView<'_> {
        self.body.field.view()
    }

    /// Borrows the original source-type text cell.
    #[must_use]
    pub const fn source_type(&self) -> &p4kx_extraction::SourcedValue<HealthTextRef> {
        self.body.source_type.cell()
    }

    /// Borrows the complete source type with this occurrence's own origin.
    #[must_use]
    pub fn source_type_text(&self) -> HealthTextView<'_> {
        self.body.source_type.view()
    }

    /// Borrows the exact original target GUID cell when applicable.
    #[must_use]
    pub const fn target_guid(&self) -> &p4kx_extraction::SourcedValue<HealthText> {
        &self.body.target_guid
    }

    /// Borrows the original physical Record-position cell.
    #[must_use]
    pub const fn record_index(&self) -> &p4kx_extraction::SourcedValue<u32> {
        self.subject.record_index()
    }

    /// Borrows the original canonical subject GUID cell.
    #[must_use]
    pub const fn record_guid(&self) -> &p4kx_extraction::SourcedValue<HealthText> {
        self.subject.record_guid()
    }

    /// Borrows the original normalized filename cell.
    #[must_use]
    pub const fn record_path(&self) -> &p4kx_extraction::SourcedValue<HealthTextRef> {
        self.subject.record_path()
    }

    /// Borrows this occurrence's paired complete subject filename.
    #[must_use]
    pub fn record_path_text(&self) -> HealthTextView<'_> {
        self.subject.record_path_text()
    }
}

/// One aggregate diagnostic with deterministic examples and sourced subject identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ExportDiagnostic {
    /// The affected kind, absent for a catalog-wide dependency.
    pub kind: Option<Kind>,
    /// The source or projection stage.
    pub stage: ExportDiagnosticStage,
    /// The stable semantic category.
    pub code: ExportDiagnosticCode,
    /// Whether this diagnostic blocks normal publication.
    pub severity: ExportDiagnosticSeverity,
    /// The source relationship when one applies.
    pub source_role: Option<ExportSourceRole>,
    /// The established warning code when one applies.
    pub warning_code: Option<String>,
    /// The complete number of affected units.
    pub count: usize,
    /// The unit of the diagnostic count.
    pub count_unit: ExportDiagnosticUnit,
    /// Deterministically retained semantic examples.
    pub(crate) examples: Vec<ExportDiagnosticExample>,
}
impl ExportDiagnostic {
    /// Borrows examples retaining this diagnostic's original report context.
    #[must_use]
    pub fn examples(&self) -> &[ExportDiagnosticExample] {
        &self.examples
    }
}
