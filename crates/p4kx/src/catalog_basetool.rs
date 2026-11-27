// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Args;
use clap::builder::{PathBufValueParser, TypedValueParser};
use p4kx_export::{BasetoolCatalogOptions, BasetoolCatalogSummary, CatalogFile};
use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken, ProvenanceMode};
use serde::Serialize;
use thiserror::Error;

use crate::{catalog_input, exit, extract, json};

#[derive(Debug, Args)]
pub(crate) struct BasetoolArgs {
    /// Path of the Data.p4k Archive.
    archive: PathBuf,
    /// The compressed .json.zst file to replace after successful verification.
    #[arg(long, value_name = "FILE", value_parser = PathBufValueParser::new().try_map(output_path))]
    out: PathBuf,
    /// The export time in seconds since 1970-01-01 UTC.
    #[arg(long, env = "SOURCE_DATE_EPOCH", value_name = "SECONDS")]
    exported_at: Option<u64>,
}

#[derive(Debug, Error)]
#[error("the Basetool catalog requires an output ending in .json.zst")]
struct InvalidOutputName;

fn output_path(path: PathBuf) -> Result<PathBuf, InvalidOutputName> {
    let zstd = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zst"));
    let json = path
        .file_stem()
        .and_then(|stem| Path::new(stem).extension())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
    if zstd && json {
        Ok(path)
    } else {
        Err(InvalidOutputName)
    }
}

pub(crate) fn run(
    args: &BasetoolArgs,
    as_json: bool,
    provenance: ProvenanceMode,
) -> Result<ExitCode> {
    catalog_input::require_basetool_markers(provenance)?;
    let exported_at = catalog_input::export_time(args.exported_at)?;
    let archive = crate::key::open(&args.archive)?;
    let cancel = extract::cancel_on_interrupt()?;
    let patches = crate::launcher_discovery::patches()?;
    let mut options = BasetoolCatalogOptions::new(exported_at);
    options.cancel = Some(&cancel);
    options.public_patches = Some(&patches);
    let summary = write_file(&archive, &args.out, &options, &cancel)?;
    print_result(&archive, &args.out, &summary, as_json)?;
    Ok(ExitCode::from(exit::SUCCESS))
}

fn write_file(
    archive: &Archive,
    out: &Path,
    options: &BasetoolCatalogOptions<'_>,
    cancel: &CancelToken,
) -> Result<BasetoolCatalogSummary> {
    let watch = ArchiveWatch::new(archive);
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    protection.ensure_writable(out, cancel.flag())?;
    let parent = out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    protection.ensure_writable(parent, cancel.flag())?;
    let (summary, file) =
        p4kx_export::write_basetool_catalog_file(archive, options, parent, || {
            CatalogFile::create_protected(out, &protection, cancel)
        })
        .map_err(|error| match error.into_catalog_error() {
            Ok(catalog) => catalog_input::into_anyhow(catalog),
            Err(reduced) => reduced.into(),
        })?;
    file.commit_if_current(&watch)
        .with_context(|| format!("cannot write {}", out.display()))?;
    Ok(summary)
}

#[derive(Serialize)]
struct SummaryJson {
    archive: String,
    out: String,
    format: &'static str,
    format_version: u32,
    size: u64,
    compressed_size: u64,
}

fn print_result(
    archive: &Archive,
    out: &Path,
    summary: &BasetoolCatalogSummary,
    as_json: bool,
) -> io::Result<()> {
    if as_json {
        json::print(&SummaryJson {
            archive: json::path(archive.path()),
            out: json::path(out),
            format: summary.format,
            format_version: summary.format_version,
            size: summary.decoded_bytes,
            compressed_size: summary.compressed_bytes,
        })
    } else {
        writeln!(
            io::stdout().lock(),
            "wrote {} ({} v{}, {} JSON bytes, {} compressed bytes)",
            out.display(),
            summary.format,
            summary.format_version,
            summary.decoded_bytes,
            summary.compressed_bytes,
        )
    }
}
