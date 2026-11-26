// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result};
use p4kx_export::{
    CatalogError, CatalogFailures, CatalogFile, CatalogOptions, CatalogPublicationMode,
    CatalogStage, CatalogSummary, Compression, CraftingWarning, ExportDiagnosticSeverity,
    ExportHealth, Filter, Kind, StarmapTraversalBudget, catalog_schema_for,
    write_catalog_file_with_phases,
};
use p4kx_extraction::{Archive, ArchiveWatch, ProvenanceMode};

use super::catalog_json::summary_json;
use super::catalog_size::warn_about_size;
use crate::catalog_input::{MissingCatalogArgs, compression, export_time};
use crate::cli::{CatalogArgs, CatalogCommand};
use crate::exit;
use crate::extract::cancel_on_interrupt;
use crate::json;
use crate::loc::print_notes;
use crate::progress::ProgressLine;

#[path = "catalog_refusal_progress.rs"]
mod refusal_progress;

/// Writes a publishable catalog or an explicitly requested diagnostic catalog and prints its summary.
/// Returns exit code 1 for incomplete diagnostic output; normal incomplete output is refused.
pub(crate) fn run(args: &CatalogArgs, as_json: bool) -> Result<ExitCode> {
    let provenance_mode = args.provenance.into();
    if matches!(args.command, Some(CatalogCommand::Schema)) {
        print_schema(provenance_mode, as_json)?;
        return Ok(ExitCode::from(exit::SUCCESS));
    }
    let (Some(archive), Some(out)) = (&args.archive, &args.out) else {
        return Err(MissingCatalogArgs.into());
    };
    let compression = compression(out, args.zstd_level)?;
    let exported_at = export_time(args.exported_at)?;
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
        progress: if args.progress_phases {
            None
        } else {
            Some(&show)
        },
        public_patches: Some(&patches),
        starmap_budget: StarmapTraversalBudget::default(),
        publication_mode: if args.diagnostic_partial {
            CatalogPublicationMode::DiagnosticPartial
        } else {
            CatalogPublicationMode::Normal
        },
    };
    crate::guard::ensure_writable(out)?;
    let phase = |stage: CatalogStage| {
        if args.progress_phases {
            let _ignored = writeln!(io::stderr().lock(), "catalog phase: {}", stage.as_str());
        }
    };
    let written = write_file(
        &archive,
        &options,
        out,
        compression,
        provenance_mode,
        &phase,
    );
    if !args.progress_phases {
        line.finish();
    }
    if args.progress_phases {
        report_outcome(&written);
    }
    let summary = &written?;
    print_result(&archive, out, summary, as_json, provenance_mode)?;
    Ok(ExitCode::from(if summary.is_complete() {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }))
}

fn report_outcome(written: &Result<CatalogSummary>) {
    match written {
        Ok(summary) => report_storage(summary),
        Err(error) => {
            if let Some(CatalogError::IncompleteExport(refusal)) = error.downcast_ref() {
                let _ignored = report_refusal(refusal.health(), &mut io::stderr().lock());
            }
        }
    }
}

fn report_storage(summary: &CatalogSummary) {
    let staged = summary.staged_row_bytes();
    let publication = summary.upload_size();
    let peak = staged + u128::from(publication);
    let _ignored = writeln!(
        io::stderr().lock(),
        "catalog storage: staged_row_bytes={staged} publication_bytes={publication} peak_temporary_bytes={peak}"
    );
}

fn report_refusal(health: &ExportHealth, output: &mut impl Write) -> io::Result<()> {
    writeln!(
        output,
        "catalog refusal: read={:?} projection={:?}",
        health.read, health.projection,
    )?;
    for diagnostic in health
        .diagnostics()
        .iter()
        .filter(|row| row.severity != ExportDiagnosticSeverity::Information)
    {
        let kind = diagnostic.kind.map_or("catalog", Kind::name);
        let warning = diagnostic.warning_code.as_deref().unwrap_or("-");
        writeln!(
            output,
            "catalog refusal: kind={kind} stage={:?} severity={:?} code={:?} warning={warning} count={} unit={:?}",
            diagnostic.stage,
            diagnostic.severity,
            diagnostic.code,
            diagnostic.count,
            diagnostic.count_unit,
        )?;
    }
    refusal_progress::report(health, output)
}

fn print_result(
    archive: &Archive,
    out: &Path,
    summary: &CatalogSummary,
    as_json: bool,
    provenance_mode: ProvenanceMode,
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
        json::print(&summary_json(archive, out, summary, provenance_mode))
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

/// Streams the catalog into a temporary file next to `out` and publishes it on success.
fn write_file(
    archive: &Archive,
    options: &CatalogOptions<'_>,
    out: &Path,
    compression: Compression,
    provenance_mode: ProvenanceMode,
    phase: &dyn Fn(CatalogStage),
) -> Result<CatalogSummary> {
    let watch = ArchiveWatch::new(archive);
    let uncancelled = p4kx_extraction::CancelToken::new();
    let cancel = options.cancel.unwrap_or(&uncancelled);
    let protection = p4kx_extraction::ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let written = write_catalog_file_with_phases(
        archive,
        options.with_provenance_mode(provenance_mode),
        compression,
        phase,
        || CatalogFile::create_protected(out, &protection, cancel),
    );
    let (summary, file) = written.map_err(into_anyhow)?;
    file.commit_if_current(&watch)
        .with_context(|| format!("cannot write {}", out.display()))?;
    Ok(summary)
}

fn print_failures(failures: CatalogFailures<'_>) -> io::Result<()> {
    let mut err = io::stderr().lock();
    for failure in failures {
        writeln!(
            err,
            "failed {} ({}): {}",
            failure.record_path(),
            failure.guid(),
            failure.reason()
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

fn print_schema(provenance_mode: ProvenanceMode, as_json: bool) -> Result<()> {
    let schema = catalog_schema_for(provenance_mode);
    if as_json {
        json::print(&schema)?;
        return Ok(());
    }
    let mut text = serde_json::to_string_pretty(&schema)?;
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
        Some(CatalogError::IncompleteExport(_))
    )
    .then_some("use --diagnostic-partial only to write an explicitly marked diagnostic catalog")
}
