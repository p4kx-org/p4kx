// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use p4kx_export::{
    CatalogError, CatalogFailure, CatalogFile, CatalogOptions, CatalogPublicationMode,
    CatalogSummary, Compression, CraftingWarning, Filter, catalog_schema, write_catalog_file,
};
use p4kx_extraction::Archive;

use super::catalog_json::summary_json;
use super::catalog_size::warn_about_size;
use crate::catalog_input::{InvalidExportTime, MissingCatalogArgs, refuse_gzip};
use crate::cli::{CatalogArgs, CatalogCommand};
use crate::exit;
use crate::extract::cancel_on_interrupt;
use crate::json;
use crate::loc::print_notes;
use crate::progress::ProgressLine;

/// Writes a publishable catalog or an explicitly requested diagnostic catalog and prints its summary.
/// Returns exit code 1 for incomplete diagnostic output; normal incomplete output is refused.
pub(crate) fn run(args: &CatalogArgs, as_json: bool) -> Result<ExitCode> {
    if matches!(args.command, Some(CatalogCommand::Schema)) {
        print_schema()?;
        return Ok(ExitCode::from(exit::SUCCESS));
    }
    let (Some(archive), Some(out)) = (&args.archive, &args.out) else {
        return Err(MissingCatalogArgs.into());
    };
    refuse_gzip(out)?;
    let exported_at = match args.exported_at {
        None => SystemTime::now(),
        Some(seconds) => UNIX_EPOCH
            .checked_add(Duration::from_secs(seconds))
            .ok_or_else(|| InvalidExportTime::of(seconds))?,
    };
    let archive = crate::key::open(archive)?;
    let filter = args.filter.as_deref().map(Filter::parse).transpose()?;
    let cancel = cancel_on_interrupt()?;
    let line = ProgressLine::new();
    let show = |progress| line.show_records(progress);
    let patches = crate::launcher_discovery::patches()?;
    let options = CatalogOptions {
        filter: filter.as_ref(),
        exported_at,
        cancel: Some(&cancel),
        progress: Some(&show),
        public_patches: Some(&patches),
        publication_mode: if args.diagnostic_partial {
            CatalogPublicationMode::DiagnosticPartial
        } else {
            CatalogPublicationMode::Normal
        },
    };
    crate::guard::ensure_writable(out)?;
    let written = write_file(&archive, &options, out);
    line.finish();
    let summary = &written?;
    print_result(&archive, out, summary, as_json)?;
    Ok(ExitCode::from(if summary.is_complete() {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }))
}

fn print_result(
    archive: &Archive,
    out: &Path,
    summary: &CatalogSummary,
    as_json: bool,
) -> Result<()> {
    if summary.export_health().publication_mode == CatalogPublicationMode::DiagnosticPartial {
        writeln!(
            io::stderr().lock(),
            "diagnostic catalog: publication mode diagnosticPartial"
        )?;
    }
    print_notes(summary.text_notes());
    print_failures(summary.failures())?;
    print_warnings(summary.warnings())?;
    warn_about_size(summary)?;
    let printed = if as_json {
        json::print(&summary_json(archive, out, summary))
    } else {
        crate::catalog_summary::print(summary, out)
    };
    if let Err(error) = printed
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(error.into());
    }
    Ok(())
}

/// Builds the catalog and streams it into a temporary file next to `out`, compressed when the name
/// ends in `.zst`, then moves it over `out`; a failure or an interrupt leaves `out` as it was.
fn write_file(
    archive: &Archive,
    options: &CatalogOptions<'_>,
    out: &Path,
) -> Result<CatalogSummary> {
    let uncancelled = p4kx_extraction::CancelToken::new();
    let cancel = options.cancel.unwrap_or(&uncancelled);
    let protection = p4kx_extraction::ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let written = write_catalog_file(archive, options, Compression::for_path(out), || {
        CatalogFile::create_protected(out, &protection, cancel)
    });
    let (summary, file) = written.map_err(into_anyhow)?;
    file.commit()
        .with_context(|| format!("cannot write {}", out.display()))?;
    Ok(summary)
}

fn print_failures(failures: &[CatalogFailure]) -> io::Result<()> {
    let mut err = io::stderr().lock();
    for failure in failures {
        writeln!(
            err,
            "failed {} ({}): {}",
            failure.record_path, failure.guid, failure.reason
        )?;
    }
    Ok(())
}

fn print_warnings(warnings: &[CraftingWarning]) -> io::Result<()> {
    let mut err = io::stderr().lock();
    for warning in warnings {
        writeln!(err, "warning: {warning}")?;
    }
    Ok(())
}

fn print_schema() -> Result<()> {
    let mut text = serde_json::to_string_pretty(&catalog_schema())?;
    text.push('\n');
    io::stdout().lock().write_all(text.as_bytes())?;
    Ok(())
}

fn into_anyhow(error: CatalogError) -> anyhow::Error {
    match error {
        CatalogError::Load(error) => error.into(),
        CatalogError::LoadTexts(error) => error.into(),
        CatalogError::DataForge(error) => error.into(),
        other => other.into(),
    }
}

/// Returns the explicit diagnostic-export option for a refused incomplete catalog.
pub(crate) fn hint(error: &anyhow::Error) -> Option<&'static str> {
    matches!(
        error.downcast_ref::<CatalogError>(),
        Some(CatalogError::IncompleteExport { .. })
    )
    .then_some("use --diagnostic-partial only to write an explicitly marked diagnostic catalog")
}
