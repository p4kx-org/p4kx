// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::health_entry_cells::EntryCells;
use super::health_entry_observation::FailedEntryObservation;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportDiagnosticStage, ExportSourceRole};
use crate::export_entry_ledger::EntryOperation;
use crate::{HealthText, HealthTextRef, HealthTextView, Kind, schema_fields};

/// One exact failed Entry operation with immutable original body cells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct FailedExportEntry {
    operation: EntryOperation,
    kind: Kind,
    #[serde(flatten)]
    body: EntryCells,
    role: ExportSourceRole,
    stage: ExportDiagnosticStage,
    code: ExportDiagnosticCode,
}

impl FailedExportEntry {
    pub(super) const fn new(observation: &FailedEntryObservation, body: EntryCells) -> Self {
        Self {
            operation: observation.source.operation,
            kind: observation.source.kind,
            body,
            role: observation.source.role,
            stage: observation.stage,
            code: observation.code,
        }
    }

    /// Returns the failed required operation.
    #[must_use]
    pub const fn operation(&self) -> EntryOperation {
        self.operation
    }

    /// Returns the affected catalog kind.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// Borrows the physical outer Entry-position cell.
    #[must_use]
    pub const fn entry_index(&self) -> &SourcedValue<usize> {
        &self.body.entry_index
    }

    /// Borrows the original nested Entry-position cell.
    #[must_use]
    pub const fn container_entry_index(&self) -> &SourcedValue<usize> {
        &self.body.container_entry_index
    }

    /// Borrows the canonical path cell in this report's text store.
    #[must_use]
    pub const fn path(&self) -> &SourcedValue<HealthTextRef> {
        self.body.path.cell()
    }

    /// Borrows the complete path with this occurrence's own origin.
    #[must_use]
    pub fn path_text(&self) -> HealthTextView<'_> {
        self.body.path.view()
    }

    /// Returns the source relationship.
    #[must_use]
    pub const fn role(&self) -> ExportSourceRole {
        self.role
    }

    /// Returns the failed stage.
    #[must_use]
    pub const fn stage(&self) -> ExportDiagnosticStage {
        self.stage
    }

    /// Returns the stable failure category.
    #[must_use]
    pub const fn code(&self) -> ExportDiagnosticCode {
        self.code
    }

    /// Borrows the diagnosed field cell.
    #[must_use]
    pub const fn field(&self) -> &SourcedValue<HealthTextRef> {
        self.body.field.cell()
    }

    /// Borrows the field text with this occurrence's own origin.
    #[must_use]
    pub fn field_text(&self) -> HealthTextView<'_> {
        self.body.field.view()
    }

    /// Borrows the technical source-type cell.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<HealthTextRef> {
        self.body.source_type.cell()
    }

    /// Borrows the source type with this occurrence's own origin.
    #[must_use]
    pub fn source_type_text(&self) -> HealthTextView<'_> {
        self.body.source_type.view()
    }

    /// Borrows the configured GUID datum when applicable.
    #[must_use]
    pub const fn target_guid(&self) -> &SourcedValue<HealthText> {
        &self.body.target_guid
    }
}
