// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::Archive;

use crate::export_diagnostic::ExportDiagnosticSeverity;
use crate::export_entry_ledger::{ArchiveEntryObservation, EntryOperation};
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::{CatalogError, ExportAccountingError};

/// The failed operation observed on an equipment resolver input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum EquipmentFailureStage {
    /// Reading the required input failed.
    SourceRead,
    /// The input was read but its required projection failed.
    Projection,
}

/// One actual failed Archive-entry operation of this query.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EquipmentEntryFailure {
    operation: EntryOperation,
    entry_index: Option<usize>,
    container_entry_index: Option<usize>,
    path: String,
    stage: EquipmentFailureStage,
}

impl EquipmentEntryFailure {
    /// Returns the actual required operation.
    #[must_use]
    pub const fn operation(&self) -> EntryOperation {
        self.operation
    }

    /// Returns the physical Entry position when present.
    #[must_use]
    pub const fn entry_index(&self) -> Option<usize> {
        self.entry_index
    }

    /// Returns the containing physical Entry position for an inner member.
    #[must_use]
    pub const fn container_entry_index(&self) -> Option<usize> {
        self.container_entry_index
    }

    /// Borrows the exact observed source lookup path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the actually failed operation stage.
    #[must_use]
    pub const fn stage(&self) -> EquipmentFailureStage {
        self.stage
    }
}

/// Actual resolver coverage, separate from the selected output-owner survey.
#[derive(Debug, Default)]
pub struct EquipmentDependencyCoverage {
    failed_records: Vec<u32>,
    failed_entries: Vec<EquipmentEntryFailure>,
    incomplete_records: Vec<u32>,
    incomplete_fields: usize,
    directory_complete: bool,
}

impl EquipmentDependencyCoverage {
    /// Borrows distinct physical Records whose required resolver operation failed.
    #[must_use]
    pub fn failed_records(&self) -> &[u32] {
        &self.failed_records
    }

    /// Borrows distinct failed Entry operations in first-observed order.
    #[must_use]
    pub fn failed_entries(&self) -> &[EquipmentEntryFailure] {
        &self.failed_entries
    }

    /// Borrows physical Records with incomplete field inspection or source coordinates.
    #[must_use]
    pub fn incomplete_records(&self) -> &[u32] {
        &self.incomplete_records
    }

    /// Returns the count of blocking original field observations, without sampling them.
    #[must_use]
    pub const fn incomplete_fields(&self) -> usize {
        self.incomplete_fields
    }

    /// Reports whether the consulted Archive directory establishes complete lookup scope.
    #[must_use]
    pub const fn directory_complete(&self) -> bool {
        self.directory_complete
    }

    /// Reports complete resolver input, field-inspection and Entry-operation coverage.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.directory_complete
            && self.failed_records.is_empty()
            && self.failed_entries.is_empty()
            && self.incomplete_records.is_empty()
            && self.incomplete_fields == 0
    }

    pub(super) fn record_failure(&mut self, record_index: u32) {
        self.failed_records.push(record_index);
    }

    pub(super) fn source_incomplete(&mut self, record_index: u32) {
        self.incomplete_records.push(record_index);
    }

    pub(super) fn finish(
        &mut self,
        archive: &Archive,
        ledger: &ExportLedger,
    ) -> Result<(), CatalogError> {
        let mut fold = EquipmentCoverageFold::default();
        fold.observe(ledger)?;
        fold.finish(archive, self)
    }
}

/// Folds resolver observations into their terminal SDK coverage.
#[derive(Default)]
pub(super) struct EquipmentCoverageFold {
    failed_records: BTreeSet<u32>,
    incomplete_records: BTreeSet<u32>,
    entry_operations: BTreeMap<EquipmentEntryFailure, usize>,
    incomplete_fields: usize,
}

impl EquipmentCoverageFold {
    pub(super) fn observe(&mut self, ledger: &ExportLedger) -> Result<(), CatalogError> {
        self.failed_records.extend(
            ledger
                .sources
                .iter()
                .filter(|row| row.disposition.failed())
                .map(|row| row.record_index),
        );
        self.failed_records
            .extend(ledger.derived_failures.iter().map(|row| row.record_index));
        self.incomplete_records.extend(
            ledger
                .field_inspections
                .iter()
                .filter(|row| !row.complete)
                .map(|row| row.record_index),
        );
        for row in &ledger.entries {
            self.observe_entry(row);
        }
        for row in &ledger.field_diagnostics {
            if row.severity != ExportDiagnosticSeverity::Information {
                self.incomplete_fields = self
                    .incomplete_fields
                    .checked_add(row.count)
                    .ok_or(ExportAccountingError::CounterOverflow)?;
            }
        }
        Ok(())
    }

    fn observe_entry(&mut self, row: &ArchiveEntryObservation) {
        let stage = match row.disposition {
            SourceDisposition::ReadFailed => EquipmentFailureStage::SourceRead,
            SourceDisposition::ProjectionFailed => EquipmentFailureStage::Projection,
            _ => return,
        };
        let failure = EquipmentEntryFailure {
            operation: row.operation,
            entry_index: row.entry_index,
            container_entry_index: row.container_entry_index,
            path: row.path.to_string(),
            stage,
        };
        let next = self.entry_operations.len();
        self.entry_operations.entry(failure).or_insert(next);
    }

    pub(super) fn finish(
        self,
        archive: &Archive,
        coverage: &mut EquipmentDependencyCoverage,
    ) -> Result<(), CatalogError> {
        coverage.directory_complete = crate::export_archive_health::inventory(archive).is_none();
        coverage.failed_records.extend(self.failed_records);
        coverage.failed_records.sort_unstable();
        coverage.failed_records.dedup();
        coverage.incomplete_records.extend(self.incomplete_records);
        coverage.incomplete_records.sort_unstable();
        coverage.incomplete_records.dedup();
        let mut failures: Vec<_> = self.entry_operations.into_iter().collect();
        failures.sort_unstable_by_key(|(_, order)| *order);
        coverage
            .failed_entries
            .extend(failures.into_iter().map(|(failure, _)| failure));
        coverage.incomplete_fields = coverage
            .incomplete_fields
            .checked_add(self.incomplete_fields)
            .ok_or(ExportAccountingError::CounterOverflow)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "equipment_dependency_coverage_tests.rs"]
mod tests;
