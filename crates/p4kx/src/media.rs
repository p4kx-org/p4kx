// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::process::ExitCode;

use p4kx_extraction::{Exclusion, Filter};
use p4kx_media::{ExportOptions, ExportOutcome, Index, MediaError};
use serde::Serialize;

use crate::cli_media::{MediaAction, MediaArgs, MediaExportArgs};
use crate::{exit, extract, json};

pub(crate) fn run(args: &MediaArgs, as_json: bool) -> anyhow::Result<ExitCode> {
    match &args.action {
        MediaAction::Schema {
            record,
            reference,
            source,
        } => {
            let schema = if *source {
                p4kx_media::source_schema()
            } else if *reference {
                p4kx_media::reference_schema()
            } else if *record {
                p4kx_media::builtin_record_schema()?
            } else {
                p4kx_media::index_schema()
            };
            json::print(&schema)?;
            Ok(ExitCode::from(exit::SUCCESS))
        }
        MediaAction::Export(args) => export(args, as_json),
    }
}

fn export(args: &MediaExportArgs, as_json: bool) -> anyhow::Result<ExitCode> {
    crate::guard::ensure_writable(&args.out)?;
    let archive = crate::key::open(&args.archive)?;
    crate::issues::warn(&archive)?;
    let options = ExportOptions {
        kinds: args.kinds.clone(),
        include: args.filter.as_deref().map(Filter::parse).transpose()?,
        exclude: Exclusion::parse(&args.exclude)?,
        conflict: extract::policy(args.conflict),
        limits: args.limits.limits(),
        cancel: extract::cancel_on_interrupt()?,
        diagnostic: args.diagnostic,
        audio_output: args.audio_format.into(),
        texture_mip: args.texture_mip,
        animation_skeleton: args.animation_skeleton.clone(),
        animation_entity: args.animation_entity.clone(),
    };
    let result = p4kx_media::export(&archive, &args.out, &options)?;
    let (status, index, code) = match &result {
        ExportOutcome::Published {
            changed: true,
            index,
        } => ("published", index.as_ref(), exit::SUCCESS),
        ExportOutcome::Published {
            changed: false,
            index,
        } => ("unchanged", index.as_ref(), exit::SUCCESS),
        ExportOutcome::Skipped { index } => ("skipped", index.as_ref(), exit::SUCCESS),
        ExportOutcome::Incomplete { index, .. } => {
            ("incomplete", index.as_ref(), exit::PARTIAL_FAILURE)
        }
        _ => {
            return Err(MediaError::Contract("unsupported media generation outcome".into()).into());
        }
    };
    print(status, index, as_json)?;
    Ok(ExitCode::from(code))
}

fn print(status: &str, index: &Index, as_json: bool) -> io::Result<()> {
    #[derive(Serialize)]
    struct Report<'a> {
        status: &'a str,
        index: &'a Index,
    }
    if as_json {
        return json::print(&Report { status, index });
    }
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "Media generation {status}: {} ({} physical Entries, {} provider streams)",
        index.generation,
        index.source_entries.written,
        index.kinds.len()
    )
}

pub(crate) fn error_code(error: &anyhow::Error) -> Option<u8> {
    code(error.downcast_ref::<MediaError>()?)
}

fn code(error: &MediaError) -> Option<u8> {
    match error {
        MediaError::Source { cause, .. } | MediaError::ExtractionInterrupted { cause, .. } => {
            code(cause)
        }
        MediaError::Output {
            cause,
            callback: None,
        } => code(cause),
        MediaError::Changed(_) => Some(exit::ARCHIVE_CHANGED),
        MediaError::Protected(cause) => Some(if cause.is_cancelled() {
            exit::PARTIAL_FAILURE
        } else {
            exit::INVALID_INPUT
        }),
        MediaError::Destination(_) => Some(exit::INVALID_INPUT),
        MediaError::Cancelled
        | MediaError::Unsupported { .. }
        | MediaError::Limit { .. }
        | MediaError::Contract(_)
        | MediaError::Extraction { .. }
        | MediaError::Archive(_)
        | MediaError::IncompleteInventory { .. }
        | MediaError::ProjectionLoss { .. }
        | MediaError::Io { .. }
        | MediaError::Cleanup(_)
        | MediaError::OwnedOutput(_)
        | MediaError::CleanupFailed { .. }
        | MediaError::InputCheckFailed { .. }
        | MediaError::Output {
            callback: Some(_), ..
        }
        | MediaError::OutputCheckFailed { .. }
        | MediaError::RetainedOutput { .. }
        | MediaError::Json(_) => Some(exit::PARTIAL_FAILURE),
        _ => None,
    }
}

#[cfg(test)]
#[path = "media_input_tests.rs"]
mod input_tests;

#[cfg(test)]
#[path = "media_output_tests.rs"]
mod output_tests;

#[cfg(test)]
#[path = "media_serialization_tests.rs"]
mod serialization_tests;
