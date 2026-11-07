// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Entry;

use crate::Kind;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_entry_ledger::{ArchiveEntryObservation, EntryOperation};
use crate::export_ledger::{ExportLedger, SourceDiagnosis, SourceDisposition};

/// The physical identity of one Archive source operation.
#[derive(Debug, Clone)]
pub(crate) struct EntrySource {
    outer: Option<usize>,
    inner: Option<usize>,
    path: String,
    operation: EntryOperation,
    role: ExportSourceRole,
}

impl EntrySource {
    /// Identifies an Entry in the source Archive.
    pub(crate) fn outer(entry: &Entry) -> Self {
        Self {
            outer: Some(entry.index()),
            inner: None,
            path: entry.path().to_owned(),
            operation: EntryOperation::ContainerDirectory,
            role: ExportSourceRole::DerivedInput,
        }
    }

    /// Identifies an Entry inside an actual source container.
    pub(crate) fn inner(outer: &Entry, inner: &Entry) -> Self {
        Self {
            outer: Some(outer.index()),
            inner: Some(inner.index()),
            path: format!("{}!{}", outer.path(), inner.path()),
            operation: EntryOperation::SourceXml,
            role: ExportSourceRole::DerivedInput,
        }
    }

    /// Retains an authored path whose configured Entry is absent.
    pub(crate) fn missing(path: &str) -> Self {
        Self {
            outer: None,
            inner: None,
            path: path.to_owned(),
            operation: EntryOperation::Enumeration,
            role: ExportSourceRole::DerivedInput,
        }
    }

    /// Identifies the required operation without changing the physical source identity.
    #[must_use]
    pub(crate) const fn operation(mut self, operation: EntryOperation) -> Self {
        self.operation = operation;
        self
    }

    /// Marks an Entry as a required dependency rather than a directly selected input.
    #[must_use]
    pub(crate) const fn dependency(mut self) -> Self {
        self.role = ExportSourceRole::Dependency;
        self
    }

    /// Records the actual result of the required read or projection.
    pub(crate) fn record(
        self,
        ledger: &mut ExportLedger,
        kind: Kind,
        disposition: SourceDisposition,
    ) {
        self.diagnosed(ledger, kind, disposition, SourceDiagnosis::default());
    }

    /// Records a source result with the available field metadata.
    pub(crate) fn diagnosed(
        self,
        ledger: &mut ExportLedger,
        kind: Kind,
        disposition: SourceDisposition,
        diagnosis: SourceDiagnosis,
    ) {
        ledger.entry(ArchiveEntryObservation {
            kind,
            entry_index: self.outer,
            container_entry_index: self.inner,
            path: self.path,
            role: self.role,
            disposition,
            diagnosis,
            operation: self.operation,
        });
    }
}

/// Copies actual Entry operations as dependencies of another consuming kind.
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
