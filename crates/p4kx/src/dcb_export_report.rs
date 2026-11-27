// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use p4kx_export::{NDJSON_FILE, RecordExportSummary, RecordFailure, RecordForm};
use p4kx_extraction::{ExtractionError, RecordExportReport, RecordRenamed};

use crate::cli::Conflict;
use crate::exit;
use crate::renamed_report::write_renamed;

/// What a Record export did, from a per-Record or NDJSON export or a combined file.
pub(crate) struct Outcome<'a> {
    pub(super) written: usize,
    pub(super) skipped: &'a [String],
    pub(super) failures: &'a [RecordFailure],
    pub(super) renamed: &'a [RecordRenamed],
    pub(super) cancelled: usize,
    pub(super) error_values: usize,
    pub(super) records_with_errors: usize,
    pub(super) error: Option<&'a dyn Error>,
    pub(super) origin_receipts: &'a [PathBuf],
    pub(super) origin_failures: &'a [ExtractionError],
}

impl Outcome<'_> {
    const fn is_complete(&self) -> bool {
        self.failures.is_empty()
            && self.cancelled == 0
            && self.error.is_none()
            && self.origin_failures.is_empty()
    }

    pub(super) fn exit_code(&self) -> ExitCode {
        ExitCode::from(if self.is_complete() {
            exit::SUCCESS
        } else {
            exit::PARTIAL_FAILURE
        })
    }
}

impl<'a> From<&'a RecordExportSummary> for Outcome<'a> {
    fn from(summary: &'a RecordExportSummary) -> Self {
        Self {
            written: summary.written(),
            skipped: summary.skipped(),
            failures: summary.failures(),
            renamed: summary.renamed(),
            cancelled: summary.cancelled(),
            error_values: summary.error_values(),
            records_with_errors: summary.records_with_errors(),
            error: summary.error().map(|error| error as &dyn Error),
            origin_receipts: &[],
            origin_failures: &[],
        }
    }
}

impl<'a> From<&'a RecordExportReport> for Outcome<'a> {
    fn from(report: &'a RecordExportReport) -> Self {
        Self {
            written: report.written(),
            skipped: report.skipped(),
            failures: report.failures(),
            renamed: report.renamed(),
            cancelled: report.cancelled(),
            error_values: report.error_values(),
            records_with_errors: report.records_with_errors(),
            error: None,
            origin_receipts: report.origin_receipts(),
            origin_failures: report.origin_failures(),
        }
    }
}

pub(super) fn reason(error: &dyn Error) -> String {
    let mut reason = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        reason.push_str(": ");
        reason.push_str(&cause.to_string());
        source = cause.source();
    }
    reason
}

/// Prints the failed Records to stderr and the kept files and a summary to stdout.
pub(crate) fn print_report(
    report: &Outcome<'_>,
    target: &Path,
    conflict: Conflict,
    form: RecordForm,
) -> io::Result<()> {
    let file = form.file().unwrap_or(NDJSON_FILE);
    let mut err = io::stderr().lock();
    for failure in report.failures {
        writeln!(
            err,
            "failed {} ({}): {}",
            failure.record_path(),
            failure.guid(),
            reason(failure.error())
        )?;
    }
    write_renamed(&mut err, report.renamed)?;
    for failure in report.origin_failures {
        writeln!(err, "Record origin: {}", reason(failure))?;
    }
    if let Some(error) = report.error {
        writeln!(err, "failed {file}: {}", reason(error))?;
    }
    let mut out = io::stdout().lock();
    let kept = if conflict == Conflict::IfChanged {
        "unchanged"
    } else {
        "already exists"
    };
    for file in report.skipped {
        writeln!(out, "skipped {file}: {kept}")?;
    }
    writeln!(
        out,
        "exported {} Records, skipped {}, failed {}, cancelled {} into {}; {} error values in {} Records",
        report.written,
        report.skipped.len(),
        report.failures.len(),
        report.cancelled,
        target.display(),
        report.error_values,
        report.records_with_errors
    )
}
