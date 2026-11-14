// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::LoadoutFileRead;
use p4kx_extraction::{Archive, Entry};

use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, EntryDiagnosis, EntryRead, EntryReceipt, LoadoutBodyReceipt,
};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_entry_ledger::{ArchiveEntryObservation, EntryOperation};
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::{CatalogError, Kind};

/// One operation retaining the exact Entry admitted before its original read.
#[derive(Debug, Clone)]
pub(crate) struct EntrySource {
    receipt: EntryReceipt,
    operation: EntryOperation,
    role: ExportSourceRole,
}

impl EntrySource {
    pub(crate) fn outer(
        capture: &EntryCapture<'_>,
        archive: &Archive,
        entry: &Entry,
    ) -> Result<Self, CatalogError> {
        Ok(Self::new(
            EntryReceipt::outer(capture, archive, entry)?,
            EntryOperation::ContainerDirectory,
        ))
    }

    pub(crate) fn loadout(
        capture: &EntryCapture<'_>,
        read: &LoadoutFileRead,
    ) -> Result<Self, CatalogError> {
        Ok(Self::new(
            EntryReceipt::loadout(capture, read)?,
            EntryOperation::SourceXml,
        ))
    }

    pub(crate) fn from_read(read: &EntryRead, operation: EntryOperation) -> Self {
        Self::new(read.receipt().clone(), operation)
    }

    pub(crate) fn of_operation(
        capture: &EntryCapture<'_>,
        body: LoadoutBodyReceipt,
        operation: EntryOperation,
    ) -> Result<Self, CatalogError> {
        Ok(Self::new(
            EntryReceipt::of_operation(capture, body)?,
            operation,
        ))
    }

    pub(crate) fn inner(
        capture: &EntryCapture<'_>,
        archive: &Archive,
        outer: &Entry,
        container: &Archive,
        inner: &Entry,
    ) -> Result<Self, CatalogError> {
        Ok(Self::new(
            EntryReceipt::inner(capture, archive, outer, container, inner)?,
            EntryOperation::SourceXml,
        ))
    }

    pub(crate) fn fixed_missing(
        capture: &EntryCapture<'_>,
        path: &'static str,
    ) -> Result<Self, CatalogError> {
        Ok(Self::new(
            EntryReceipt::fixed(capture, path)?,
            EntryOperation::Enumeration,
        ))
    }

    pub(crate) fn authored_missing(path: AuthoredEntryPath) -> Self {
        Self::new(EntryReceipt::selected(path), EntryOperation::Enumeration)
    }

    const fn new(receipt: EntryReceipt, operation: EntryOperation) -> Self {
        Self {
            receipt,
            operation,
            role: ExportSourceRole::DerivedInput,
        }
    }

    pub(crate) const fn receipt(&self) -> &EntryReceipt {
        &self.receipt
    }

    pub(crate) fn merge(&mut self, other: &Self) -> Result<(), crate::ExportAccountingError> {
        if self.operation != other.operation || self.role != other.role {
            return Err(crate::ExportAccountingError::ConflictingSourceDatum);
        }
        self.receipt.merge(&other.receipt)
    }

    #[must_use]
    pub(crate) const fn operation(mut self, operation: EntryOperation) -> Self {
        self.operation = operation;
        self
    }

    #[must_use]
    pub(crate) const fn dependency(mut self) -> Self {
        self.role = ExportSourceRole::Dependency;
        self
    }

    pub(crate) fn record(
        self,
        ledger: &mut ExportLedger,
        kind: Kind,
        disposition: SourceDisposition,
    ) {
        self.diagnosed(ledger, kind, disposition, EntryDiagnosis::default());
    }

    pub(crate) fn diagnosed(
        self,
        ledger: &mut ExportLedger,
        kind: Kind,
        disposition: SourceDisposition,
        diagnosis: EntryDiagnosis,
    ) {
        ledger.entry(self.observation(kind, disposition, diagnosis));
    }

    pub(crate) fn observation(
        self,
        kind: Kind,
        disposition: SourceDisposition,
        diagnosis: EntryDiagnosis,
    ) -> ArchiveEntryObservation {
        ArchiveEntryObservation {
            kind,
            entry_index: self.receipt.entry_index(),
            container_entry_index: self.receipt.container_entry_index(),
            path: self.receipt.path_storage(),
            role: self.role,
            disposition,
            diagnosis,
            operation: self.operation,
            receipt: Some(self.receipt),
        }
    }
}

/// Copies actual operation receipts to another consuming kind without another source read.
pub(crate) fn dependencies(source: &ExportLedger, ledger: &mut ExportLedger, kind: Kind) {
    if !source.incomplete_inventories.is_empty() {
        ledger.incomplete_inventory(kind);
    }
    for mut entry in source.entries.iter().cloned() {
        entry.kind = kind;
        entry.role = ExportSourceRole::Dependency;
        ledger.entry(entry);
    }
}
