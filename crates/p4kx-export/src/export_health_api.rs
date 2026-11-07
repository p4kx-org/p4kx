// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Public source-health, accounting and diagnostic contracts.

pub use crate::export_diagnostic::{
    EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT, ExportDiagnostic, ExportDiagnosticCode,
    ExportDiagnosticExample, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole, FailedExportRecord,
};
pub use crate::export_entry_counts::ExportEntryCounts;
pub use crate::export_entry_ledger::{EntryOperation, FailedExportEntry};
pub use crate::export_health::{
    CatalogPublicationMode, ExportHealth, ExportHealthStatus, ExportSourceCounts, ExportSourceMode,
    ExportStageStatus, KindExportHealth,
};
pub use crate::export_ledger::ExportAccountingError;
pub use crate::export_reference_issue::{
    ExportReferenceStatus, ExportSourcePointer, ExportUnresolvedReference,
};
