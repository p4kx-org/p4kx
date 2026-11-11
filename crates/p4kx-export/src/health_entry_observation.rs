// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::export_diagnostic::{ExportDiagnosticCode, ExportDiagnosticStage};
use crate::export_entry_ledger::ArchiveEntryObservation;

/// A failed operation retained before its original Entry cells are materialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FailedEntryObservation {
    pub(crate) source: ArchiveEntryObservation,
    pub(crate) stage: ExportDiagnosticStage,
    pub(crate) code: ExportDiagnosticCode,
}
