// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use p4kx_assembly::audio::{
    ExportReport, ExportState, TriggerSounds, TxtpApproximation, TxtpReport, write_txtp,
};
use serde::Serialize;

use crate::cli_wwise::WwiseExtractArgs;
use crate::extract::policy;
use crate::json;

/// The JSON shape of the TXTP files of an export.
#[derive(Serialize)]
pub(crate) struct TxtpJson {
    written: usize,
    kept: usize,
    skipped: usize,
    files: Vec<TxtpFileJson>,
    skips: Vec<TxtpSkipJson>,
}

#[derive(Serialize)]
struct TxtpFileJson {
    trigger: String,
    event: String,
    file: String,
    size: u64,
    kept: bool,
    breaks: usize,
    approximations: Option<Vec<&'static str>>,
}

#[derive(Serialize)]
struct TxtpSkipJson {
    trigger: String,
    event: String,
    reason: String,
}

/// Writes the TXTP files of `sounds` next to the sounds `exported` holds.
pub(crate) fn write(
    sounds: &TriggerSounds<'_>,
    exported: &ExportReport,
    args: &WwiseExtractArgs,
) -> TxtpReport {
    write_txtp(&[sounds], exported, &args.out, policy(args.export.conflict))
}

/// Returns the JSON shape of `report`.
pub(crate) fn to_json(report: &TxtpReport) -> TxtpJson {
    TxtpJson {
        written: report.written_count(),
        kept: report.kept_count(),
        skipped: report.skipped.len(),
        files: report
            .files
            .iter()
            .map(|file| TxtpFileJson {
                trigger: file.trigger.clone(),
                event: file.event.clone(),
                file: json::path(&file.path),
                size: file.size,
                kept: file.state == ExportState::Kept,
                breaks: file.breaks,
                approximations: file.approximations.as_ref().map(|reasons| {
                    reasons
                        .iter()
                        .copied()
                        .map(TxtpApproximation::code)
                        .collect()
                }),
            })
            .collect(),
        skips: report
            .skipped
            .iter()
            .map(|skip| TxtpSkipJson {
                trigger: skip.trigger.clone(),
                event: skip.event.clone(),
                reason: skip.reason.clone(),
            })
            .collect(),
    }
}

/// Prints the summary of the TXTP files to stdout and the skipped events to stderr.
pub(crate) fn print(report: &TxtpReport, out: &Path) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    for skip in &report.skipped {
        writeln!(
            stderr,
            "error: {}: {}: no TXTP written: {}",
            skip.trigger, skip.event, skip.reason
        )?;
    }
    writeln!(
        io::stdout().lock(),
        "wrote {} TXTP files into {}; {} kept, {} incomplete because of broken links, {} events skipped",
        report.written_count(),
        out.display(),
        report.kept_count(),
        report.files.iter().filter(|file| file.breaks > 0).count(),
        report.skipped.len()
    )
}
