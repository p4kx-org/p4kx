// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::catalog_reference::CatalogReference;
use crate::export_diagnostic::{ExportDiagnostic, ExportSourceRole};
use crate::export_reference_issue::ExportUnresolvedReference;
use crate::selection::Kind;

/// One bounded inspection of an actual canonical tree, independently of its emitted rows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub(crate) struct FieldInspection {
    pub(crate) kind: Option<Kind>,
    pub(crate) record_index: u32,
    pub(crate) guid: Guid,
    pub(crate) record_path: String,
    pub(crate) role: ExportSourceRole,
    pub(crate) complete: bool,
    pub(crate) diagnostics: Vec<ExportDiagnostic>,
    /// Conditional failures until the exact physical source has an emitted lossless row.
    pub(crate) resource_number_diagnostics: Vec<ExportDiagnostic>,
    pub(crate) unresolved_references: Vec<ExportUnresolvedReference>,
    pub(crate) catalog_references: Vec<CatalogReference>,
}

impl FieldInspection {
    /// Reuses the same immutable canonical inspection for an explicit source role or output kind.
    pub(crate) fn contextualize(&mut self, kind: Option<Kind>, role: ExportSourceRole) {
        self.kind = kind;
        self.role = role;
        for diagnostic in self
            .diagnostics
            .iter_mut()
            .chain(&mut self.resource_number_diagnostics)
        {
            diagnostic.kind = kind;
            diagnostic.source_role = Some(role);
        }
        for reference in &mut self.unresolved_references {
            reference.kind = kind;
            reference.source_role = role;
        }
    }
}
