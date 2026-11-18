// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::Archive;

use crate::export_diagnostic::ExportDiagnosticSeverity;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::{CatalogError, ExportAccountingError};

/// The failed operation observed on a Ship resolver input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ShipSourceFailureStage {
    /// Reading the required input failed.
    SourceRead,
    /// The input was read but its required projection failed.
    Projection,
}

/// One actual failed Archive-entry operation of this query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipSourceEntryFailure {
    operation: EntryOperation,
    entry_index: Option<usize>,
    container_entry_index: Option<usize>,
    path: String,
    stage: ShipSourceFailureStage,
}

impl ShipSourceEntryFailure {
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
    pub const fn stage(&self) -> ShipSourceFailureStage {
        self.stage
    }
}

/// Actual resolver coverage, separate from the selected output-owner survey.
#[derive(Debug, Default)]
pub struct ShipSourceDependencyCoverage {
    failed_records: Vec<u32>,
    failed_entries: Vec<ShipSourceEntryFailure>,
    incomplete_records: Vec<u32>,
    incomplete_fields: usize,
    directory_complete: bool,
}

impl ShipSourceDependencyCoverage {
    /// Borrows distinct physical Records whose required resolver operation failed.
    #[must_use]
    pub fn failed_records(&self) -> &[u32] {
        &self.failed_records
    }

    /// Borrows distinct failed Entry operations in first-observed order.
    #[must_use]
    pub fn failed_entries(&self) -> &[ShipSourceEntryFailure] {
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
        self.directory_complete = crate::export_archive_health::inventory(archive).is_none();
        self.failed_records.extend(
            ledger
                .sources
                .iter()
                .filter(|row| row.disposition.failed())
                .map(|row| row.record_index),
        );
        self.failed_records
            .extend(ledger.derived_failures.iter().map(|row| row.record_index));
        self.failed_records.sort_unstable();
        self.failed_records.dedup();
        self.incomplete_records.extend(
            ledger
                .field_inspections
                .iter()
                .filter(|row| !row.complete)
                .map(|row| row.record_index),
        );
        self.incomplete_records.sort_unstable();
        self.incomplete_records.dedup();
        let mut entry_operations = BTreeSet::new();
        for row in &ledger.entries {
            let stage = match row.disposition {
                SourceDisposition::ReadFailed => ShipSourceFailureStage::SourceRead,
                SourceDisposition::ProjectionFailed => ShipSourceFailureStage::Projection,
                _ => continue,
            };
            if !entry_operations.insert((
                row.operation,
                row.entry_index,
                row.container_entry_index,
                row.path.as_str(),
                stage,
            )) {
                continue;
            }
            let failure = ShipSourceEntryFailure {
                operation: row.operation,
                entry_index: row.entry_index,
                container_entry_index: row.container_entry_index,
                path: row.path.to_string(),
                stage,
            };
            self.failed_entries.push(failure);
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
}
