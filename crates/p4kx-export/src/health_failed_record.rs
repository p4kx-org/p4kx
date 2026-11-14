// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::health_record_cells::RecordCells;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportDiagnosticStage, ExportSourceRole};
use crate::health_metadata::HealthTextRef;
use crate::selection::Kind;
use crate::{HealthText, HealthTextView, schema_fields};

/// One exact failed source identity, retained independently of diagnostic samples.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct FailedExportRecord {
    /// The affected kind, absent for a catalog-wide source or dependency.
    pub kind: Option<Kind>,
    #[serde(flatten)]
    header: RecordCells,
    /// The relationship of this source to the affected kind.
    pub role: ExportSourceRole,
    /// The failed stage.
    pub stage: ExportDiagnosticStage,
    /// The stable failure category.
    pub code: ExportDiagnosticCode,
}

impl FailedExportRecord {
    pub(super) const fn new(
        kind: Option<Kind>,
        header: RecordCells,
        role: ExportSourceRole,
        stage: ExportDiagnosticStage,
        code: ExportDiagnosticCode,
    ) -> Self {
        Self {
            kind,
            header,
            role,
            stage,
            code,
        }
    }

    /// Borrows the original physical position cell.
    #[must_use]
    pub const fn record_index(&self) -> &SourcedValue<u32> {
        self.header.index()
    }

    /// Borrows the original canonical GUID cell.
    #[must_use]
    pub const fn guid(&self) -> &SourcedValue<HealthText> {
        self.header.guid()
    }

    /// Borrows the original normalized filename cell.
    #[must_use]
    pub const fn record_path(&self) -> &SourcedValue<HealthTextRef> {
        self.header.path()
    }

    /// Borrows the original technical type cell.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<HealthTextRef> {
        self.header.source_type()
    }

    /// Borrows this occurrence's paired complete filename text.
    #[must_use]
    pub fn record_path_text(&self) -> HealthTextView<'_> {
        self.header.path_text()
    }

    /// Borrows this occurrence's paired technical type text.
    #[must_use]
    pub fn source_type_text(&self) -> HealthTextView<'_> {
        self.header.type_text()
    }
}
