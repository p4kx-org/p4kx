// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{FailedEntryObservation, RecordSubject, ReferenceObservation};
use crate::catalog_read_failure::CatalogReadFailure;
use crate::export_entry_counts::ExportEntryCounts;
use crate::export_entry_ledger::ArchiveEntryObservation;
use crate::export_health::{
    CatalogPublicationMode, ExportHealth, ExportHealthStatus, ExportSourceCounts, ExportSourceMode,
    ExportStageStatus, KindExportHealth,
};
use crate::health_diagnostic_observation::DiagnosticObservation;
use crate::health_failed_observation::FailedRecordObservation;
use crate::{CatalogError, FailedExportEntry, FailedExportRecord, Kind};

/// One kind's reconciled accounting, without unbound public source cells.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct KindFacts {
    pub(crate) kind: Kind,
    pub(crate) source_mode: ExportSourceMode,
    pub(crate) source_inventory_complete: bool,
    pub(crate) field_inventory_complete: bool,
    pub(crate) read: ExportStageStatus,
    pub(crate) projection: ExportStageStatus,
    pub(crate) source_records: ExportSourceCounts,
    pub(crate) source_entries: ExportEntryCounts,
    pub(crate) written_rows: usize,
    pub(crate) intentionally_skipped_rows: usize,
    pub(crate) failed_derived_projections: usize,
    pub(crate) unresolved_reference_count: usize,
}
impl KindFacts {
    fn finish(
        self,
        records: &[FailedExportRecord],
        entries: &[FailedExportEntry],
    ) -> KindExportHealth {
        KindExportHealth {
            kind: self.kind,
            source_mode: self.source_mode,
            source_inventory_complete: self.source_inventory_complete,
            field_inventory_complete: self.field_inventory_complete,
            read: self.read,
            projection: self.projection,
            source_records: self.source_records,
            source_entries: self.source_entries,
            written_rows: self.written_rows,
            intentionally_skipped_rows: self.intentionally_skipped_rows,
            failed_derived_projections: self.failed_derived_projections,
            unresolved_reference_count: self.unresolved_reference_count,
            failed_records: records
                .iter()
                .filter(|record| record.kind == Some(self.kind))
                .cloned()
                .collect(),
            failed_entries: entries
                .iter()
                .filter(|entry| entry.kind() == self.kind)
                .cloned()
                .collect(),
        }
    }
}

/// Private accounting and actual subjects awaiting this preparation's original input seal.
#[derive(Debug)]
pub(crate) struct PreparedHealth {
    pub(crate) owners: Vec<RecordSubject>,
    pub(crate) entry_owners: Vec<ArchiveEntryObservation>,
    pub(crate) status: ExportHealthStatus,
    pub(crate) read: ExportStageStatus,
    pub(crate) projection: ExportStageStatus,
    pub(crate) publication_mode: CatalogPublicationMode,
    pub(crate) unique_selected_source_records: usize,
    pub(crate) selected_kind_records: usize,
    pub(crate) written_rows: usize,
    pub(crate) kinds: Vec<KindFacts>,
    pub(crate) failed_records: Vec<FailedRecordObservation>,
    pub(crate) failed_entries: Vec<FailedEntryObservation>,
    pub(crate) unresolved_references: Vec<ReferenceObservation>,
    pub(crate) diagnostics: Vec<DiagnosticObservation>,
}
impl PreparedHealth {
    #[cfg(test)]
    pub(crate) fn kind(&self, kind: Kind) -> Option<&KindFacts> {
        self.kinds.iter().find(|facts| facts.kind == kind)
    }
    #[cfg(test)]
    pub(crate) const fn is_complete(&self) -> bool {
        matches!(self.status, ExportHealthStatus::Complete)
    }
    #[cfg(test)]
    pub(crate) const fn permits_publication(&self, mode: CatalogPublicationMode) -> bool {
        self.is_complete() || matches!(mode, CatalogPublicationMode::DiagnosticPartial)
    }
    pub(crate) fn finish(
        self,
        input: &OriginalCatalogInput<'_, '_>,
        failures: &[CatalogReadFailure<'_>],
        cancel: Option<&CancelToken>,
    ) -> Result<ExportHealth, CatalogError> {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        for owner in &self.owners {
            owner.check_input(input)?;
        }
        for owner in &self.entry_owners {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            owner.original()?.check_input(input)?;
        }
        let captured = crate::catalog_read::original::finish_report(
            input,
            failures,
            self.failed_records,
            self.failed_entries,
            self.diagnostics,
            self.unresolved_references,
            cancel,
        )?;
        let kinds = self
            .kinds
            .into_iter()
            .map(|kind| kind.finish(&captured.failed_records, &captured.failed_entries))
            .collect();
        Ok(ExportHealth {
            metadata: captured.metadata,
            failures: captured.failures,
            status: self.status,
            read: self.read,
            projection: self.projection,
            publication_mode: self.publication_mode,
            unique_selected_source_records: self.unique_selected_source_records,
            selected_kind_records: self.selected_kind_records,
            written_rows: self.written_rows,
            kinds,
            failed_records: captured.failed_records,
            failed_entries: captured.failed_entries,
            unresolved_references: captured.references,
            diagnostics: captured.diagnostics,
        })
    }
}
// Accounting equality never admits the separately retained original input bindings.
impl PartialEq for PreparedHealth {
    fn eq(&self, other: &Self) -> bool {
        self.status == other.status
            && self.read == other.read
            && self.projection == other.projection
            && self.publication_mode == other.publication_mode
            && self.unique_selected_source_records == other.unique_selected_source_records
            && self.selected_kind_records == other.selected_kind_records
            && self.written_rows == other.written_rows
            && self.kinds == other.kinds
            && self.failed_records == other.failed_records
            && self.failed_entries == other.failed_entries
            && self.unresolved_references == other.unresolved_references
            && self.diagnostics == other.diagnostics
    }
}
impl Eq for PreparedHealth {}
