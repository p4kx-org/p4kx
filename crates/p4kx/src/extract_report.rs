// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::{EntryFailure, ExtractionReport, TextureArtifacts};
use serde::Serialize;

use crate::cli::Conflict;
use crate::dcb_export;
use crate::failure_report::{FailureJson, MismatchJson, reason, write_mismatch};
use crate::json;
use crate::renamed_report::{RenamedJson, renamed_json, write_renamed};

#[derive(Serialize)]
pub(crate) struct ReportJson<'a> {
    target: String,
    extracted: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    converted: Option<usize>,
    skipped: &'a [String],
    failed: Vec<FailureJson<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mismatched: Option<Vec<MismatchJson<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unconverted: Option<Vec<FailureJson<'a>>>,
    cancelled: &'a [String],
    #[serde(skip_serializing_if = "is_zero")]
    not_converted: usize,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    dataforge_left_out: &'a [String],
    archive_changed: bool,
    #[serde(skip_serializing_if = "<[TextureArtifacts]>::is_empty")]
    texture_artifacts: &'a [TextureArtifacts],
    #[serde(skip_serializing_if = "Option::is_none")]
    records: Option<RecordsJson<'a>>,
}

#[derive(Serialize)]
struct RecordsJson<'a> {
    exported: usize,
    skipped: &'a [String],
    failed: Vec<dcb_export::FailureJson<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    renamed: Vec<RenamedJson<'a>>,
    cancelled: usize,
    error_values: usize,
    records_with_errors: usize,
}

pub(crate) fn report_json<'a>(
    report: &'a ExtractionReport,
    target: &Path,
    verify: bool,
) -> ReportJson<'a> {
    ReportJson {
        target: json::path(target),
        extracted: report.extracted(),
        converted: report.converted(),
        skipped: report.skipped(),
        failed: failures_json(report.failures(), verify),
        mismatched: verify.then(|| {
            report
                .failures()
                .iter()
                .filter_map(MismatchJson::of)
                .collect()
        }),
        unconverted: report
            .converted()
            .map(|_| failures_json(report.unconverted(), false)),
        cancelled: report.cancelled(),
        not_converted: report.not_converted(),
        dataforge_left_out: report.dataforge_left_out(),
        archive_changed: report.archive_changed(),
        texture_artifacts: report.texture_artifacts(),
        records: report.records().map(|records| RecordsJson {
            exported: records.written(),
            skipped: records.skipped(),
            failed: records
                .failures()
                .iter()
                .map(dcb_export::failure_json)
                .collect(),
            renamed: records.renamed().iter().map(renamed_json).collect(),
            cancelled: records.cancelled(),
            error_values: records.error_values(),
            records_with_errors: records.records_with_errors(),
        }),
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes a reference"
)]
const fn is_zero(count: &usize) -> bool {
    *count == 0
}

fn failures_json(failures: &[EntryFailure], without_mismatches: bool) -> Vec<FailureJson<'_>> {
    failures
        .iter()
        .filter(|failure| !(without_mismatches && failure.checksum_mismatch().is_some()))
        .map(FailureJson::of)
        .collect()
}

pub(crate) fn print_report(
    report: &ExtractionReport,
    target: &Path,
    conflict: Conflict,
    verify: bool,
) -> io::Result<()> {
    write_errors(&mut io::stderr().lock(), report)?;
    let mut out = io::stdout().lock();
    let kept = if conflict == Conflict::IfChanged {
        "unchanged"
    } else {
        "already exists"
    };
    for path in report.skipped() {
        writeln!(out, "skipped {path}: {kept}")?;
    }
    for path in report.cancelled() {
        writeln!(out, "cancelled {path}")?;
    }
    let converted = match (report.converted(), report.unconverted().len()) {
        (None, _) => String::new(),
        (Some(count), 0) => format!(" ({count} converted)"),
        (Some(count), unconverted) => format!(" ({count} converted, {unconverted} not converted)"),
    };
    let bad = report.checksum_mismatches().count();
    let (failed, mismatches) = if verify {
        (
            report.failures().len() - bad,
            format!(", checksum mismatches {bad}"),
        )
    } else {
        (report.failures().len(), String::new())
    };
    let left_out = match report.not_converted() {
        0 => String::new(),
        count => format!(", left out {count} without a conversion"),
    };
    writeln!(
        out,
        "extracted {}{converted}, skipped {}, failed {failed}{mismatches}, cancelled {}{left_out} \
         into {}",
        report.extracted(),
        report.skipped().len(),
        report.cancelled().len(),
        target.display()
    )
}

/// Writes the failed, not converted and left-out files of `report` to `err`.
fn write_errors(err: &mut impl Write, report: &ExtractionReport) -> io::Result<()> {
    for failure in report.failures() {
        if failure.checksum_mismatch().is_some() {
            write_mismatch(err, failure)?;
        } else {
            writeln!(err, "failed {}: {}", failure.path(), reason(failure))?;
        }
    }
    if let Some(records) = report.records() {
        write_renamed(err, records.renamed())?;
    }
    for unconverted in report.unconverted() {
        writeln!(
            err,
            "not converted {}: {}",
            unconverted.path(),
            reason(unconverted)
        )?;
    }
    for path in report.dataforge_left_out() {
        writeln!(err, "left out {path}: another DataForge file was converted")?;
    }
    Ok(())
}
