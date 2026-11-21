// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use p4kx_assembly::audio::{
    ExportLoopOrigins, ExportReport, ExportState, TriggerSounds, TxtpApproximation, TxtpReport,
    write_txtp_with_loop_origins,
};
use p4kx_extraction::{Archive, CancelToken, FieldOrigins, SourceInventory};
use serde::Serialize;

use crate::cli_wwise::WwiseExtractArgs;
use crate::extract::policy;
use crate::json;

/// The JSON shape of the TXTP files of an export.
#[derive(Serialize)]
pub(crate) struct TxtpJson<'a> {
    written: usize,
    kept: usize,
    skipped: usize,
    files: Vec<TxtpFileJson<'a>>,
    skips: Vec<TxtpSkipJson>,
}

#[derive(Serialize)]
struct TxtpFileJson<'a> {
    trigger: String,
    event: String,
    file: String,
    size: u64,
    kept: bool,
    breaks: usize,
    approximations: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commands: Option<Vec<SoundCommandJson>>,
    #[serde(rename = "fieldOrigins", skip_serializing_if = "Option::is_none")]
    field_origins: Option<&'a FieldOrigins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provenance: Option<&'a SourceInventory>,
    #[serde(rename = "originReceipt")]
    origin_receipt: Option<String>,
}

#[derive(Serialize)]
struct SoundCommandJson {
    #[serde(rename = "loop", skip_serializing_if = "Option::is_none")]
    loop_points: Option<LoopJson>,
}

#[derive(Serialize)]
struct LoopJson {
    start: u64,
    end: u64,
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
    archive: &Archive,
    cancel: &CancelToken,
    loop_origins: &ExportLoopOrigins,
) -> io::Result<TxtpReport> {
    write_txtp_with_loop_origins(
        archive,
        &[sounds],
        exported,
        &args.out,
        policy(args.export.conflict),
        cancel,
        loop_origins,
    )
}

/// Returns the JSON shape of `report`.
pub(crate) fn to_json(report: &TxtpReport) -> TxtpJson<'_> {
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
                commands: file.command_loops.as_ref().map(|commands| {
                    commands
                        .iter()
                        .map(|points| SoundCommandJson {
                            loop_points: points.map(|points| LoopJson {
                                start: points.start,
                                end: points.end,
                            }),
                        })
                        .collect()
                }),
                field_origins: file.origins.as_ref().map(|origins| &origins.field_origins),
                provenance: file.origins.as_ref().map(|origins| &origins.provenance),
                origin_receipt: file
                    .origin_receipt
                    .as_deref()
                    .and_then(Path::file_name)
                    .and_then(std::ffi::OsStr::to_str)
                    .map(str::to_owned),
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
