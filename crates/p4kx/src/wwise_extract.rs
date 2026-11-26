// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;
use p4kx_assembly::audio::{
    ArchiveAudio, AudioIndex, ExportOptions, ExportReport, ExportReportCapture, ExportState,
    ExportWatch, SoundFormat, TriggerSounds, TxtpReport,
};
use p4kx_extraction::{CancelToken, ConversionError, FieldOrigins, SourceInventory};
use serde::Serialize;

use crate::cli::AudioOutput;
use crate::cli_wwise::{SoundExportArgs, WwiseExtractArgs};
use crate::entity_audio::{IssueJson, issues_json, warn};
use crate::extract::{cancel_on_interrupt, policy};
use crate::wwise::{UnknownTrigger, language_choice};
use crate::wwise_json::{self, LanguageJson, TriggerJson};
use crate::wwise_txtp::{self, TxtpJson};
use crate::{exit, json};

#[derive(Serialize)]
struct ExtractJson<'a> {
    archive: String,
    language: LanguageJson<'a>,
    trigger: TriggerJson<'a>,
    breaks: usize,
    #[serde(flatten)]
    export: ExportJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    txtp: Option<TxtpJson<'a>>,
    issues: Vec<IssueJson<'a>>,
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

/// The JSON shape of a sound export.
#[derive(Serialize)]
pub(crate) struct ExportJson {
    target: String,
    format: &'static str,
    written: usize,
    kept: usize,
    unconverted: usize,
    failed: usize,
    cancelled: bool,
    files: Vec<FileJson>,
    failures: Vec<FailureJson>,
}

#[derive(Serialize)]
struct FileJson {
    trigger: String,
    wem_id: u32,
    file: String,
    origin_receipt: Option<String>,
    origin_receipt_failure: Option<String>,
    format: &'static str,
    size: u64,
    kept: bool,
    unconverted: Option<String>,
}

#[derive(Serialize)]
struct FailureJson {
    trigger: String,
    wem_id: Option<u32>,
    reason: String,
}

const fn sound_format(output: AudioOutput) -> SoundFormat {
    match output {
        AudioOutput::Ogg => SoundFormat::Ogg,
        AudioOutput::Wav => SoundFormat::Wav,
        AudioOutput::Raw => SoundFormat::Raw,
    }
}

/// Writes the sounds of `triggers` into `out` as the export arguments say.
pub(crate) fn export(
    audio: &ArchiveAudio<'_>,
    triggers: &[&TriggerSounds<'_>],
    out: &Path,
    args: &SoundExportArgs,
    cancel: &CancelToken,
    capture_report: bool,
) -> Result<ExportReport> {
    let watch = ExportWatch::linked(cancel.clone());
    let options = export_options(args);
    if capture_report {
        return Ok(audio
            .export_sounds_watched_with_report_origins(
                triggers,
                out,
                options,
                &watch,
                ExportReportCapture::FileSizes,
            )?
            .0);
    }
    Ok(audio.export_sounds_watched(triggers, out, options, &watch)?)
}

const fn export_options(args: &SoundExportArgs) -> ExportOptions {
    ExportOptions {
        format: sound_format(args.format),
        policy: policy(args.conflict),
    }
}

/// Returns the JSON shape of `report`.
pub(crate) fn export_json(report: &ExportReport, out: &Path, args: &SoundExportArgs) -> ExportJson {
    ExportJson {
        target: json::path(out),
        format: match args.format {
            AudioOutput::Ogg => "ogg",
            AudioOutput::Wav => "wav",
            AudioOutput::Raw => "raw",
        },
        written: report.written_count(),
        kept: report.kept_count(),
        unconverted: report.unconverted_count(),
        failed: report.failures.len(),
        cancelled: report.cancelled,
        files: report
            .sounds
            .iter()
            .map(|sound| FileJson {
                trigger: sound.trigger.clone(),
                wem_id: sound.wem_id,
                file: json::path(&sound.path),
                origin_receipt: sound.origin_receipt.as_deref().map(json::path),
                origin_receipt_failure: sound.origin_receipt_failure.clone(),
                format: sound.format.extension(),
                size: sound.size,
                kept: sound.state == ExportState::Kept,
                unconverted: sound.unconverted.clone(),
            })
            .collect(),
        failures: report
            .failures
            .iter()
            .map(|failure| FailureJson {
                trigger: failure.trigger.clone(),
                wem_id: failure.wem_id,
                reason: failure.reason.clone(),
            })
            .collect(),
    }
}

/// Prints the summary of an export to stdout and what went wrong to stderr.
pub(crate) fn print_report(report: &ExportReport, out: &Path) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    for sound in &report.sounds {
        if let Some(reason) = &sound.origin_receipt_failure {
            writeln!(
                stderr,
                "error: could not publish the origin receipt for {} ({reason})",
                sound.path.display()
            )?;
        }
        if let Some(reason) = &sound.unconverted {
            writeln!(
                stderr,
                "error: {} WEM {} written unchanged as {}: {reason}",
                sound.trigger,
                sound.wem_id,
                sound.path.display()
            )?;
        }
    }
    for failure in &report.failures {
        let wem = failure
            .wem_id
            .map_or_else(String::new, |id| format!(" WEM {id}"));
        writeln!(
            stderr,
            "error: {}{wem}: {}",
            failure.trigger, failure.reason
        )?;
    }
    if report.cancelled {
        writeln!(
            stderr,
            "error: cancelled; the remaining sounds were not exported"
        )?;
    }
    writeln!(
        io::stdout().lock(),
        "wrote {} sounds into {}; {} kept, {} written unchanged because they cannot be converted, {} failed",
        report.written_count(),
        out.display(),
        report.kept_count(),
        report.unconverted_count(),
        report.failures.len()
    )
}

/// Returns the exit code of an export: partial failure unless every sound was exported as asked.
pub(crate) fn exit_code(report: &ExportReport) -> ExitCode {
    ExitCode::from(if report.is_clean() {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    })
}

/// Runs `p4kx wwise extract`.
pub(crate) fn run(args: &WwiseExtractArgs, as_json: bool) -> Result<ExitCode> {
    let cancel = cancel_on_interrupt()?;
    let archive = crate::key::open(&args.archive)?;
    let audio = ArchiveAudio::new(&archive);
    let index = audio
        .index_until(&cancel)
        .ok_or(ConversionError::Cancelled)?;
    let choice = language_choice(index, &args.resolve)?;
    let mut options = args.resolve.options(&cancel);
    if args.txtp || as_json {
        options = options.with_path_sources();
    }
    let sounds = audio
        .resolve_trigger_with_options(&args.trigger, &choice, options)?
        .ok_or_else(|| UnknownTrigger::new(&args.trigger))?;
    let (report, txtp) = if args.txtp {
        export_with_txtp(&audio, &sounds, args, &cancel, as_json)?
    } else {
        (
            export(
                &audio,
                &[&sounds],
                &args.out,
                &args.export,
                &cancel,
                as_json,
            )?,
            None,
        )
    };
    if as_json {
        let (field_origins, provenance) =
            wwise_json::capture_extract(&archive, index, &sounds, &report, &args.export)?;
        json::print(&ExtractJson {
            archive: json::path(archive.path()),
            language: LanguageJson::of(&choice),
            trigger: wwise_json::trigger(&sounds),
            breaks: sounds.breaks().len(),
            export: export_json(&report, &args.out, &args.export),
            txtp: txtp.as_ref().map(wwise_txtp::to_json),
            issues: issues_json(index),
            field_origins,
            provenance,
        })?;
    } else {
        print_extraction(
            &report,
            txtp.as_ref(),
            index,
            &args.out,
            sounds.breaks().len(),
        )?;
    }
    Ok(exit_code(&report))
}

fn export_with_txtp(
    audio: &ArchiveAudio<'_>,
    sounds: &TriggerSounds<'_>,
    args: &WwiseExtractArgs,
    cancel: &CancelToken,
    capture_report: bool,
) -> Result<(ExportReport, Option<TxtpReport>)> {
    let watch = ExportWatch::linked(cancel.clone());
    let options = export_options(&args.export);
    let (report, origins) = if capture_report {
        audio.export_sounds_watched_with_report_origins(
            &[sounds],
            &args.out,
            options,
            &watch,
            ExportReportCapture::FileSizesAndLoops,
        )?
    } else {
        audio.export_sounds_watched_with_loop_origins(&[sounds], &args.out, options, &watch)?
    };
    let txtp = (!report.cancelled)
        .then(|| wwise_txtp::write(sounds, &report, args, audio.archive(), cancel, &origins))
        .transpose()?;
    Ok((report, txtp))
}

fn print_extraction(
    report: &ExportReport,
    txtp: Option<&TxtpReport>,
    index: &AudioIndex,
    out: &Path,
    breaks: usize,
) -> io::Result<()> {
    warn(index)?;
    print_report(report, out)?;
    if let Some(txtp) = txtp {
        wwise_txtp::print(txtp, out)?;
    }
    if breaks != 0 {
        writeln!(
            io::stderr().lock(),
            "warning: {breaks} broken links; run `p4kx wwise resolve` to list them"
        )?;
    }
    Ok(())
}
