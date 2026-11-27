// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_export::RecordForm;
use p4kx_extraction::{
    DataForgeOrigin, RecordFailure, RecordRenamed, RenameCause, SourceEvidenceError,
};

use super::provenance::RecordExportProvenance;
use crate::model::describe;

/// One Record a Record export did not write, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedRecord {
    /// The record path, empty when it cannot be read.
    pub record_path: String,
    /// The GUID in canonical form.
    pub guid: String,
    /// The error and its causes, joined by colons.
    pub reason: String,
}

impl FailedRecord {
    pub(crate) fn of(failure: &RecordFailure) -> Self {
        Self {
            record_path: failure.record_path().to_owned(),
            guid: failure.guid().to_string(),
            reason: describe(failure.error()),
        }
    }
}

/// One Record written, but under a file that does not follow its name or record path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamedRecord {
    /// The record path as the DataForge states it.
    pub record_path: String,
    /// The GUID in canonical form.
    pub guid: String,
    /// Why the file differs.
    pub cause: RenameCause,
}

impl RenamedRecord {
    /// Returns the note of `renamed`.
    pub(crate) fn of(renamed: &RecordRenamed) -> Self {
        Self {
            record_path: renamed.record_path().to_owned(),
            guid: renamed.guid().to_string(),
            cause: renamed.cause(),
        }
    }
}

/// The outcome of a finished Record export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordExportSummary {
    form: RecordForm,
    target: Option<PathBuf>,
    selected: usize,
    written: usize,
    skipped: Vec<String>,
    failures: Vec<FailedRecord>,
    renamed: Vec<RenamedRecord>,
    cancelled: usize,
    error_values: usize,
    records_with_errors: usize,
    error: Option<String>,
    origin_receipts: Vec<PathBuf>,
    origin_failures: Vec<String>,
    interrupted: bool,
    provenance: Option<Result<Box<RecordExportProvenance>, SourceEvidenceError>>,
}

impl RecordExportSummary {
    pub(crate) fn new(
        target: PathBuf,
        selected: usize,
        mut summary: p4kx_export::RecordExportSummary,
        source: Option<&DataForgeOrigin>,
    ) -> Self {
        let provenance = summary
            .take_receipt()
            .map(|receipt| RecordExportProvenance::capture(source, receipt).map(Box::new));
        Self {
            form: summary.form(),
            target: Some(target),
            selected,
            written: summary.written(),
            skipped: summary.skipped().to_vec(),
            failures: summary.failures().iter().map(FailedRecord::of).collect(),
            renamed: summary.renamed().iter().map(RenamedRecord::of).collect(),
            cancelled: summary.cancelled(),
            error_values: summary.error_values(),
            records_with_errors: summary.records_with_errors(),
            error: summary.error().map(|error| describe(error)),
            origin_receipts: summary.origin_receipts().to_vec(),
            origin_failures: summary
                .origin_failures()
                .iter()
                .map(|error| describe(error))
                .collect(),
            interrupted: summary.cancelled() > 0,
            provenance,
        }
    }

    /// Returns the summary of an export that created no target and wrote nothing: none of the
    /// `selected` Records, because none matched or, when `interrupted`, because it was cancelled
    /// first.
    pub(crate) const fn nothing_written(
        form: RecordForm,
        selected: usize,
        interrupted: bool,
    ) -> Self {
        Self {
            form,
            target: None,
            selected,
            written: 0,
            skipped: Vec::new(),
            failures: Vec::new(),
            renamed: Vec::new(),
            cancelled: if interrupted { selected } else { 0 },
            error_values: 0,
            records_with_errors: 0,
            error: None,
            origin_receipts: Vec::new(),
            origin_failures: Vec::new(),
            interrupted,
            provenance: None,
        }
    }

    /// Returns the form the Records were written in.
    #[must_use]
    pub const fn form(&self) -> RecordForm {
        self.form
    }

    /// Returns the folder the Records were written into, or `None` when the export ended before
    /// it prepared one.
    #[must_use]
    pub fn target(&self) -> Option<&Path> {
        self.target.as_deref()
    }

    /// Returns the file of a single-file export (NDJSON or flat values), or `None` for one file
    /// per Record or without a target.
    #[must_use]
    pub fn file(&self) -> Option<PathBuf> {
        let target = self.target.as_deref()?;
        self.form.file().map(|file| target.join(file))
    }

    /// Returns the number of Records the export was asked to write.
    #[must_use]
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// Returns the number of Records written.
    #[must_use]
    pub const fn written(&self) -> usize {
        self.written
    }

    /// Returns the published companions of the newly written files.
    #[must_use]
    pub fn origin_receipts(&self) -> &[PathBuf] {
        &self.origin_receipts
    }

    /// Reports companion publication failures separately from committed Record files.
    #[must_use]
    pub fn origin_failures(&self) -> &[String] {
        &self.origin_failures
    }

    /// Returns the original DataForge contributors to the number of Records written.
    #[must_use]
    pub fn provenance(&self) -> Option<Result<&RecordExportProvenance, &SourceEvidenceError>> {
        self.provenance.as_ref().map(Result::as_deref)
    }

    /// Returns the files, relative to the target, whose existing file was kept.
    #[must_use]
    pub fn skipped(&self) -> &[String] {
        &self.skipped
    }

    /// Returns the Records that failed, with the reason.
    #[must_use]
    pub fn failures(&self) -> &[FailedRecord] {
        &self.failures
    }

    /// Returns the Records written under a file that differs from their name or record path; they
    /// do not count as failed.
    #[must_use]
    pub fn renamed(&self) -> &[RenamedRecord] {
        &self.renamed
    }

    /// Returns the number of Records not written because the export was cancelled.
    #[must_use]
    pub const fn cancelled(&self) -> usize {
        self.cancelled
    }

    /// Returns the number of error values in the Records written or kept.
    #[must_use]
    pub const fn error_values(&self) -> usize {
        self.error_values
    }

    /// Returns the number of Records written or kept that hold at least one error value.
    #[must_use]
    pub const fn records_with_errors(&self) -> usize {
        self.records_with_errors
    }

    /// Returns why the NDJSON file could not be written, if it could not.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Returns whether the export was cancelled before every Record was written.
    #[must_use]
    pub const fn was_cancelled(&self) -> bool {
        self.interrupted
    }
}
