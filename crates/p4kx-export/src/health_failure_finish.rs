// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;

use super::catalog_failure_details::{CapturedFailure, CatalogFailureDetails};
use super::health_entry_observation::FailedEntryObservation;
use super::health_entry_retention::RetainedEntry;
use super::health_failed_entry::FailedExportEntry;
use super::health_reference_finish::RetainedReference;
use super::health_reference_observation::ReferenceObservation;
use super::health_report_retention::{RetainedDiagnostic, RetainedRecord};
use super::health_text_capture::SealedTexts;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::export_diagnostic::{ExportDiagnostic, FailedExportRecord};
use crate::health_diagnostic_observation::DiagnosticObservation;
use crate::health_failed_observation::FailedRecordObservation;
use crate::health_metadata::HealthMetadata;
use crate::{CatalogError, ExportUnresolvedReference};

/// All semantic health occurrences sharing this report's single immutable text-store seal.
pub(crate) struct FinishedReport {
    pub(crate) metadata: HealthMetadata,
    pub(crate) failures: Vec<CatalogFailureDetails>,
    pub(crate) failed_records: Vec<FailedExportRecord>,
    pub(crate) failed_entries: Vec<FailedExportEntry>,
    pub(crate) diagnostics: Vec<ExportDiagnostic>,
    pub(crate) references: Vec<ExportUnresolvedReference>,
}

pub(crate) fn finish_report(
    input: &OriginalCatalogInput<'_, '_>,
    failures: &[CatalogReadFailure<'_>],
    failed_records: Vec<FailedRecordObservation>,
    failed_entries: Vec<FailedEntryObservation>,
    diagnostics: Vec<DiagnosticObservation>,
    references: Vec<ReferenceObservation>,
    cancel: Option<&CancelToken>,
) -> Result<FinishedReport, CatalogError> {
    checkpoint(cancel)?;
    let sources = &input.origins().uses;
    let failures = collect(failures, cancel, |failure| {
        CapturedFailure::read(input, failure)
    })?;
    let records = collect(failed_records, cancel, |record| {
        record.subject().check_input(input)?;
        RetainedRecord::read(record, sources)
    })?;
    let entries = collect(failed_entries, cancel, |entry| {
        RetainedEntry::read(entry, input)
    })?;
    let diagnostics = collect(diagnostics, cancel, |diagnostic| {
        for example in &diagnostic.examples {
            example.subject().check_input(input)?;
        }
        RetainedDiagnostic::read(diagnostic, input)
    })?;
    let references = collect(references, cancel, |reference| {
        RetainedReference::read(reference, input)
    })?;
    let texts = seal(
        input,
        &failures,
        &records,
        &entries,
        &diagnostics,
        &references,
        cancel,
    )?;
    let failures = collect(failures, cancel, |failure| failure.finish(&texts, sources))?;
    let failed_records = collect(records, cancel, |record| {
        record
            .finish(&texts, sources)
            .map_err(CatalogError::Accounting)
    })?;
    let failed_entries = collect(entries, cancel, |entry| {
        entry
            .finish(&texts, sources)
            .map_err(CatalogError::Accounting)
    })?;
    let diagnostics = collect(diagnostics, cancel, |diagnostic| {
        diagnostic
            .finish(&texts, sources)
            .map_err(CatalogError::Accounting)
    })?;
    let references = collect(references, cancel, |reference| {
        reference
            .finish(&texts, sources)
            .map_err(CatalogError::Accounting)
    })?;
    Ok(FinishedReport {
        metadata: texts.into_metadata(),
        failures,
        failed_records,
        failed_entries,
        diagnostics,
        references,
    })
}

fn seal(
    input: &OriginalCatalogInput<'_, '_>,
    failures: &[CapturedFailure],
    records: &[RetainedRecord],
    entries: &[RetainedEntry],
    diagnostics: &[RetainedDiagnostic],
    references: &[RetainedReference],
    cancel: Option<&CancelToken>,
) -> Result<SealedTexts, CatalogError> {
    let retained: Vec<_> = failures
        .iter()
        .flat_map(CapturedFailure::texts)
        .chain(records.iter().flat_map(RetainedRecord::texts))
        .chain(entries.iter().flat_map(RetainedEntry::texts))
        .chain(diagnostics.iter().flat_map(RetainedDiagnostic::texts))
        .chain(references.iter().flat_map(RetainedReference::texts))
        .collect();
    input
        .origins()
        .health_texts()
        .seal(&input.origins().uses, cancel, &retained)
}

fn collect<T, R>(
    items: impl IntoIterator<Item = T>,
    cancel: Option<&CancelToken>,
    mut map: impl FnMut(T) -> Result<R, CatalogError>,
) -> Result<Vec<R>, CatalogError> {
    items
        .into_iter()
        .map(|item| {
            checkpoint(cancel)?;
            map(item)
        })
        .collect()
}
#[cfg(test)]
pub(crate) fn finish_failures(
    input: &OriginalCatalogInput<'_, '_>,
    failures: &[CatalogReadFailure<'_>],
    cancel: Option<&CancelToken>,
) -> Result<FinishedReport, CatalogError> {
    finish_report(
        input,
        failures,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        cancel,
    )
}

#[cfg(test)]
#[path = "health_failure_capture_tests.rs"]
mod tests;
fn checkpoint(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}
