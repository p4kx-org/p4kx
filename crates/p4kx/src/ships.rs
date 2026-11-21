// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use p4kx_export::{
    ArchiveKeyState, BuiltShipBase, CatalogFailure, Filter, ShipBaseError, ShipBaseOptions,
    build_ship_base,
};
use serde::Serialize;

use crate::catalog_input::InvalidExportTime;
use crate::cli::{ShipsArgs, ShipsFormat};
use crate::exit;
use crate::extract::cancel_on_interrupt;
use crate::json;
use crate::progress::ProgressLine;

#[derive(Serialize)]
struct SummaryJson<'a> {
    archive: String,
    out: String,
    format: &'static str,
    archive_key: ArchiveKeyState,
    encrypted_entries_unread: u64,
    ships: usize,
    skipped: usize,
    mass: usize,
    hull: usize,
    cargo: usize,
    issues: usize,
    failed: Vec<FailureJson<'a>>,
}

#[derive(Serialize)]
struct FailureJson<'a> {
    guid: String,
    path: &'a str,
    error: &'a str,
}

/// Writes the base data of the ships of an Archive and prints its summary; the exit code is 1
/// when a ship could not be read.
pub(crate) fn run(args: &ShipsArgs, as_json: bool) -> Result<ExitCode> {
    let exported_at = match args.exported_at {
        None => SystemTime::now(),
        Some(seconds) => UNIX_EPOCH
            .checked_add(Duration::from_secs(seconds))
            .ok_or_else(|| InvalidExportTime::of(seconds))?,
    };
    crate::guard::ensure_writable(&args.out)?;
    let archive = crate::key::open(&args.archive)?;
    let filter = args.filter.as_deref().map(Filter::parse).transpose()?;
    let cancel = cancel_on_interrupt()?;
    let protection = p4kx_extraction::ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let line = ProgressLine::new();
    let show = |progress| line.show_records(progress);
    let patches = crate::launcher_discovery::patches()?;
    let options = ShipBaseOptions {
        filter: filter.as_ref(),
        exported_at,
        cancel: Some(&cancel),
        progress: Some(&show),
        public_patches: Some(&patches),
    };
    let built = build_ship_base(&archive, &options);
    line.finish();
    let built = built.map_err(into_anyhow)?;
    print_failures(built.failures())?;
    write(&built, args.format, &args.out, &protection, &cancel)?;
    let printed = if as_json {
        json::print(&summary(&built, args, json::path(archive.path())))
    } else {
        print_summary(&built, &args.out)
    };
    if let Err(error) = printed
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(error.into());
    }
    Ok(ExitCode::from(if built.failures().is_empty() {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }))
}

fn write(
    built: &BuiltShipBase,
    format: ShipsFormat,
    out: &Path,
    protection: &p4kx_extraction::ArchiveWriteProtection,
    cancel: &p4kx_extraction::CancelToken,
) -> Result<()> {
    crate::query_file::write_replacing_protected(out, Some(protection), Some(cancel), |writer| {
        match format {
            ShipsFormat::Json => built.write_json(writer),
            ShipsFormat::Ndjson => built.write_ndjson(writer),
        }
        .map_err(io::Error::other)
    })
    .with_context(|| format!("cannot write {}", out.display()))
}

fn into_anyhow(error: ShipBaseError) -> anyhow::Error {
    match error {
        ShipBaseError::Load(error) => error.into(),
        ShipBaseError::DataForge(error) => error.into(),
        other => other.into(),
    }
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

fn summary<'a>(built: &'a BuiltShipBase, args: &ShipsArgs, archive: String) -> SummaryJson<'a> {
    let ships = built.ships();
    SummaryJson {
        archive,
        out: json::path(&args.out),
        format: match args.format {
            ShipsFormat::Json => "json",
            ShipsFormat::Ndjson => "ndjson",
        },
        archive_key: built.archive_key().state(),
        encrypted_entries_unread: built.archive_key().entries_unread(),
        ships: ships.len(),
        skipped: built.skipped(),
        mass: ships.iter().filter(|ship| ship.mass.is_some()).count(),
        hull: ships.iter().filter(|ship| ship.hull.is_some()).count(),
        cargo: ships
            .iter()
            .filter(|ship| !ship.cargo.grids.is_empty())
            .count(),
        issues: ships.iter().filter(|ship| !ship.issues.is_empty()).count(),
        failed: built
            .failures()
            .iter()
            .map(|failure| FailureJson {
                guid: failure.guid.to_string(),
                path: &failure.record_path,
                error: &failure.reason,
            })
            .collect(),
    }
}

fn print_summary(built: &BuiltShipBase, out: &Path) -> io::Result<()> {
    let ships = built.ships();
    let count = |test: fn(&p4kx_export::ShipBase) -> bool| ships.iter().filter(|s| test(s)).count();
    writeln!(
        io::stdout().lock(),
        "wrote {}: {} ships ({} with mass, {} with hull, {} with cargo grids, {} with issues), {} Records skipped, {} failed",
        out.display(),
        ships.len(),
        count(|ship| ship.mass.is_some()),
        count(|ship| ship.hull.is_some()),
        count(|ship| !ship.cargo.grids.is_empty()),
        count(|ship| !ship.issues.is_empty()),
        built.skipped(),
        built.failures().len()
    )
}
