// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::build::original_input::OriginalCatalogInput;
use crate::export_health_compile;
#[cfg(test)]
use crate::export_ledger::ExportAccountingError;
use crate::export_ledger::ExportLedger;
use crate::{
    CatalogError, CatalogPublicationMode, CatalogReadFailure, CraftingWarning, ExportHealth,
    KindSummary,
};
use p4kx_extraction::CancelToken;

/// Compiles accounting and the source cells captured for this exact report.
pub(crate) fn compile_captured(
    ledger: ExportLedger,
    rows: &[KindSummary],
    failures: &[CatalogReadFailure<'_>],
    warnings: &[CraftingWarning],
    mode: CatalogPublicationMode,
    input: &OriginalCatalogInput<'_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<ExportHealth, CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    export_health_compile::prepare_report(
        ledger,
        rows,
        failures.iter().map(CatalogReadFailure::guid),
        warnings,
        mode,
    )?
    .finish(input, failures, cancel)
}

#[cfg(test)]
pub(crate) fn compile(
    ledger: ExportLedger,
    rows: &[KindSummary],
    failures: &[crate::CatalogFailure],
    warnings: &[CraftingWarning],
    mode: CatalogPublicationMode,
) -> Result<crate::export_health_prepared::PreparedHealth, ExportAccountingError> {
    export_health_compile::prepare_report(
        ledger,
        rows,
        failures.iter().map(|failure| failure.guid),
        warnings,
        mode,
    )
}
