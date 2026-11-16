// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ExtractionError, RecordExportReport, RecordFailure, RecordRenamed};

use crate::RecordForm;

/// Original DataForge definition indices counted as written by a committed output.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RecordExportReceipt {
    pub(crate) indices: Vec<u32>,
    pub(crate) rowless_indices: Vec<u32>,
}

impl RecordExportReceipt {
    /// Returns the definitions counted as written in file or selection order.
    #[must_use]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// Returns the counted definitions that produced no rows in a flat form.
    #[must_use]
    pub fn rowless_indices(&self) -> &[u32] {
        &self.rowless_indices
    }

    /// Takes the definitions counted as written without copying.
    #[must_use]
    pub fn into_indices(self) -> Vec<u32> {
        self.indices
    }
}

/// The outcome of a Record export.
#[derive(Debug, Default)]
pub struct RecordExportSummary {
    pub(crate) form: RecordForm,
    pub(crate) written: usize,
    pub(crate) skipped: Vec<String>,
    pub(crate) failures: Vec<RecordFailure>,
    pub(crate) renamed: Vec<RecordRenamed>,
    pub(crate) cancelled: usize,
    pub(crate) error_values: usize,
    pub(crate) records_with_errors: usize,
    pub(crate) error: Option<ExtractionError>,
    pub(crate) receipt: Option<RecordExportReceipt>,
}

impl RecordExportSummary {
    pub(crate) fn from_report(form: RecordForm, mut report: RecordExportReport) -> Self {
        let mut summary = Self {
            form,
            written: report.written(),
            skipped: report.skipped().to_vec(),
            renamed: report.renamed().to_vec(),
            cancelled: report.cancelled(),
            error_values: report.error_values(),
            records_with_errors: report.records_with_errors(),
            receipt: report
                .take_written_record_indices()
                .map(|indices| RecordExportReceipt {
                    indices,
                    rowless_indices: Vec::new(),
                }),
            ..Self::default()
        };
        summary.failures = report.into_failures();
        summary
    }

    /// Returns the form the Records were written in.
    #[must_use]
    pub const fn form(&self) -> RecordForm {
        self.form
    }

    /// Returns the number of Records written: files for one file per Record, lines for NDJSON.
    #[must_use]
    pub const fn written(&self) -> usize {
        self.written
    }

    /// Returns the committed Record positions when the run requested them.
    #[must_use]
    pub const fn receipt(&self) -> Option<&RecordExportReceipt> {
        self.receipt.as_ref()
    }

    /// Takes the committed Record positions when the run requested them.
    #[must_use]
    pub const fn take_receipt(&mut self) -> Option<RecordExportReceipt> {
        self.receipt.take()
    }

    pub(crate) fn clear_receipt(&mut self) {
        if let Some(receipt) = &mut self.receipt {
            receipt.indices.clear();
            receipt.rowless_indices.clear();
        }
    }

    /// Returns the files, relative to the target, whose existing file was kept.
    #[must_use]
    pub fn skipped(&self) -> &[String] {
        &self.skipped
    }

    /// Returns the Records that failed, with the reason.
    #[must_use]
    pub fn failures(&self) -> &[RecordFailure] {
        &self.failures
    }

    /// Returns the Records whose file does not follow their name or record path because it is no
    /// valid file name on both Linux and Windows.
    #[must_use]
    pub fn renamed(&self) -> &[RecordRenamed] {
        &self.renamed
    }

    /// Returns the number of Records not written because the run was cancelled.
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
    pub const fn error(&self) -> Option<&ExtractionError> {
        self.error.as_ref()
    }

    /// Returns whether no Record failed, the file was written and the run was not cancelled.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.failures.is_empty() && self.cancelled == 0 && self.error.is_none()
    }
}
