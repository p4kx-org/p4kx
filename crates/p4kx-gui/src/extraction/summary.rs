// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_extraction::{Entry, EntryFailure, ExtractionReport, RecordExportReport};

use crate::model::describe;
use crate::record_export::{FailedRecord, RenamedRecord};

/// One Entry an Extraction did not write, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedEntry {
    /// The path of the Entry inside the Archive.
    pub path: String,
    /// The error and its causes, joined by colons.
    pub reason: String,
}

/// The counts of the Records a Conversion wrote for the DataForge Entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordCounts {
    /// Records written.
    pub written: usize,
    /// Records whose existing file was kept.
    pub skipped: usize,
    /// Records not written because they failed.
    pub failed: usize,
    /// Records not written because the Extraction was cancelled.
    pub cancelled: usize,
    /// Error values in the Records written or kept.
    pub error_values: usize,
    /// Records written or kept that hold at least one error value.
    pub records_with_errors: usize,
}

impl RecordCounts {
    fn new(report: &RecordExportReport) -> Self {
        Self {
            written: report.written(),
            skipped: report.skipped().len(),
            failed: report.failures().len(),
            cancelled: report.cancelled(),
            error_values: report.error_values(),
            records_with_errors: report.records_with_errors(),
        }
    }
}

/// The outcome of a finished Extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionSummary {
    target: PathBuf,
    extracted: usize,
    converted: Option<usize>,
    skipped: Vec<String>,
    failures: Vec<FailedEntry>,
    unconverted: Vec<FailedEntry>,
    cancelled: Vec<String>,
    archive_changed: bool,
    records: Option<RecordCounts>,
    record_failures: Vec<FailedRecord>,
    records_renamed: Vec<RenamedRecord>,
    excluded_parts: usize,
    not_converted: usize,
}

impl ExtractionSummary {
    /// Lists the selected Entries as cancelled before a target was prepared.
    pub(crate) fn cancelled_selection(
        target: PathBuf,
        entries: &[&Entry],
        convert: bool,
        archive_changed: bool,
    ) -> Self {
        let mut summary = Self::new(target, &ExtractionReport::default());
        summary.cancelled = entries
            .iter()
            .map(|entry| entry.path().to_owned())
            .collect();
        summary.converted = convert.then_some(0);
        summary.archive_changed = archive_changed;
        summary
    }

    /// Returns the outcome of a conversion cancelled while its input folder was read: nothing
    /// was written, and the input folder is the one thing listed as cancelled.
    pub(crate) fn cancelled_scan(target: PathBuf, input: &Path) -> Self {
        let mut summary = Self::new(target, &ExtractionReport::default());
        summary.cancelled = vec![input.display().to_string()];
        summary
    }

    pub(crate) fn new(target: PathBuf, report: &ExtractionReport) -> Self {
        Self {
            target,
            extracted: report.extracted(),
            converted: report.converted(),
            skipped: report.skipped().to_vec(),
            failures: failed_entries(report.failures()),
            unconverted: failed_entries(report.unconverted()),
            cancelled: report.cancelled().to_vec(),
            archive_changed: report.archive_changed(),
            records: report.records().map(RecordCounts::new),
            record_failures: report.records().map_or_default(|records| {
                records.failures().iter().map(FailedRecord::of).collect()
            }),
            records_renamed: report.records().map_or_default(|records| {
                records.renamed().iter().map(RenamedRecord::of).collect()
            }),
            excluded_parts: 0,
            not_converted: report.not_converted(),
        }
    }

    /// Returns the folder the Entries were written into.
    #[must_use]
    pub fn target(&self) -> &Path {
        &self.target
    }

    /// Returns the number of Entries written.
    #[must_use]
    pub const fn extracted(&self) -> usize {
        self.extracted
    }

    /// Returns the number of Entries written converted, `CryXml` as XML, the Entries of converted
    /// textures and the DataForge Entry as Records, or `None` when Conversion was off.
    #[must_use]
    pub const fn converted(&self) -> Option<usize> {
        self.converted
    }

    /// Returns the paths of Entries whose existing file was kept.
    #[must_use]
    pub fn skipped(&self) -> &[String] {
        &self.skipped
    }

    /// Returns the Entries that failed, with the reason.
    #[must_use]
    pub fn failures(&self) -> &[FailedEntry] {
        &self.failures
    }

    /// Returns the textures, by their header Entry, that could not be converted, with the
    /// reason; their raw Entries were written instead.
    #[must_use]
    pub fn unconverted(&self) -> &[FailedEntry] {
        &self.unconverted
    }

    /// Returns the paths of Entries not written because the Extraction was cancelled.
    #[must_use]
    pub fn cancelled(&self) -> &[String] {
        &self.cancelled
    }

    /// Returns the counts of the Records written for the DataForge Entry, or `None` when no
    /// DataForge Entry was converted.
    #[must_use]
    pub const fn records(&self) -> Option<RecordCounts> {
        self.records
    }

    /// Returns the Records of the DataForge Entry that were not written, with the reason.
    #[must_use]
    pub fn record_failures(&self) -> &[FailedRecord] {
        &self.record_failures
    }

    /// Returns the Records of the DataForge Entry written under a file that differs from their
    /// name or record path; they do not count as failed.
    #[must_use]
    pub fn records_renamed(&self) -> &[RenamedRecord] {
        &self.records_renamed
    }

    /// Returns whether the Archive changed on disk, which stopped the Extraction.
    #[must_use]
    pub const fn archive_changed(&self) -> bool {
        self.archive_changed
    }

    /// Returns the number of part Entries the exclusion left out of textures that stayed; a
    /// Conversion read them all the same.
    #[must_use]
    pub const fn excluded_parts(&self) -> usize {
        self.excluded_parts
    }

    /// Returns the number of files a folder conversion left out because they have nothing to
    /// convert.
    #[must_use]
    pub const fn not_converted(&self) -> usize {
        self.not_converted
    }

    pub(crate) const fn with_excluded_parts(mut self, excluded_parts: usize) -> Self {
        self.excluded_parts = excluded_parts;
        self
    }
}

fn failed_entries(failures: &[EntryFailure]) -> Vec<FailedEntry> {
    failures
        .iter()
        .map(|failure| FailedEntry {
            path: failure.path().to_owned(),
            reason: describe(failure.error()),
        })
        .collect()
}
