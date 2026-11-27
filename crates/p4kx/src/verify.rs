// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_extraction::{
    Archive, ArchiveWatch, Entry, VerificationReport, VerifyOptions, verify, verify_observed,
};
use serde::Serialize;

use crate::cli::VerifyArgs;
use crate::extract;
use crate::failure_report::{FailureJson, MismatchJson, reason, unreadable, write_mismatch};
use crate::progress::ProgressLine;
use crate::{exit, filter_pair, json};

#[path = "verify_origins.rs"]
mod origins;

use origins::VerificationOrigins;

#[derive(Serialize)]
struct ReportJson<'a> {
    archive: String,
    verified: usize,
    mismatched: Vec<MismatchJson<'a>>,
    failed: Vec<FailureJson<'a>>,
    cancelled: &'a [String],
    archive_changed: bool,
    #[serde(flatten)]
    origins: VerificationOrigins,
}

/// Checks the Entries of the Archive, or those the Filter matches, against their checksums and
/// prints every mismatch; exits with 1 when an Entry mismatches or cannot be read.
pub(crate) fn run(args: &VerifyArgs, as_json: bool) -> Result<ExitCode> {
    let archive = crate::key::open(&args.archive)?;
    crate::issues::warn(&archive)?;
    let entries = filter_pair::verification(&archive, args.filter.as_deref(), &args.exclude)?;
    execute(&archive, &entries, as_json)
}

/// Verifies `entries` of `archive`, which may be a container, and prints the report.
pub(crate) fn execute(archive: &Archive, entries: &[&Entry], as_json: bool) -> Result<ExitCode> {
    crate::key::require(archive, entries)?;
    let watch = ArchiveWatch::new(archive);
    let cancel = extract::cancel_on_interrupt()?;
    let line = ProgressLine::new();
    let on_progress = |progress| line.show_verifying(progress);
    let options = VerifyOptions {
        cancel: Some(&cancel),
        progress: Some(&on_progress),
        watch: Some(&watch),
    };
    let (status, printed) = if as_json {
        let observed = verify_observed(archive, entries, &options);
        line.finish();
        let report = observed.report();
        let origins = VerificationOrigins::capture(archive, &observed)?;
        (
            exit_code(report),
            json::print(&report_json(archive, report, origins)),
        )
    } else {
        let report = verify(archive, entries, &options);
        line.finish();
        (exit_code(&report), print_report(archive, &report))
    };
    if let Err(error) = printed
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(error.into());
    }
    if status == exit::ARCHIVE_CHANGED
        && let Err(changed) = watch.check()
    {
        let _ignored = writeln!(io::stderr().lock(), "error: {changed}");
    }
    Ok(ExitCode::from(status))
}

const fn exit_code(report: &VerificationReport) -> u8 {
    if report.archive_changed() {
        exit::ARCHIVE_CHANGED
    } else if report.is_complete() {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }
}

fn report_json<'a>(
    archive: &Archive,
    report: &'a VerificationReport,
    origins: VerificationOrigins,
) -> ReportJson<'a> {
    ReportJson {
        archive: json::path(archive.location()),
        verified: report.verified(),
        mismatched: report
            .failures()
            .iter()
            .filter_map(MismatchJson::of)
            .collect(),
        failed: unreadable(report.failures()).map(FailureJson::of).collect(),
        cancelled: report.cancelled(),
        archive_changed: report.archive_changed(),
        origins,
    }
}

fn print_report(archive: &Archive, report: &VerificationReport) -> io::Result<()> {
    let mut err = io::stderr().lock();
    for failure in report.failures() {
        match failure.checksum_mismatch() {
            Some(_) => write_mismatch(&mut err, failure)?,
            None => writeln!(err, "failed {}: {}", failure.path(), reason(failure))?,
        }
    }
    let mut out = io::stdout().lock();
    for path in report.cancelled() {
        writeln!(out, "cancelled {path}")?;
    }
    let mismatched = report.checksum_mismatches().count();
    writeln!(
        out,
        "verified {}, mismatched {mismatched}, failed {}, cancelled {} in {}",
        report.verified(),
        report.failures().len() - mismatched,
        report.cancelled().len(),
        archive.location().display()
    )
}
