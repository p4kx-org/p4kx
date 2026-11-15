// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::Result;
use p4kx_extraction::{Exclusion, Filter};
use p4kx_search::{
    EmptyNeedle, Needle, SearchError, SearchOptions, SearchReport, SearchStats, Source, Sources,
    search, search_observed,
};
use thiserror::Error;

use crate::cli::{GrepArgs, GrepFormat};
use crate::dcb_refs::JsonWithTable;
use crate::{exit, grep_json, grep_table};

/// The text to search for is empty.
#[derive(Debug, Error)]
#[error("the text to search for is empty")]
pub(crate) struct EmptySearchText(#[from] EmptyNeedle);

/// Searches the Archive for the text and writes the hits to stdout and the notes to stderr.
pub(crate) fn run(args: &GrepArgs, as_json: bool) -> Result<ExitCode> {
    let json = match (as_json, args.format) {
        (true, Some(GrepFormat::Table)) => return Err(JsonWithTable.into()),
        (as_json, format) => as_json || format == Some(GrepFormat::Json),
    };
    let needle = Needle::new(&args.text).map_err(EmptySearchText)?;
    let options = options(args)?;
    let archive = crate::key::open(&args.archive)?;
    let started = Instant::now();
    let (report, elapsed) = if json {
        let observed = search_observed(&archive, &needle, &options, &|| false)
            .map_err(failure)?
            .unwrap_or_default();
        let elapsed = started.elapsed();
        grep_json::print(&args.archive, &args.text, &observed, elapsed)?;
        (observed.into_report(), elapsed)
    } else {
        let report = search(&archive, &needle, &options, &|| false)
            .map_err(failure)?
            .unwrap_or_default();
        let elapsed = started.elapsed();
        grep_table::print(&report)?;
        (report, elapsed)
    };
    notes(&report, elapsed, &options)?;
    Ok(ExitCode::from(exit::SUCCESS))
}

/// Returns the error in the form the exit codes know: the DataForge's own.
fn failure(error: SearchError) -> anyhow::Error {
    match error {
        SearchError::DataForge(error) => error.into(),
        SearchError::DataForgeOpen(error) => error.into(),
        other => other.into(),
    }
}

/// Returns the options the arguments ask for.
///
/// # Errors
///
/// Returns the [`FilterError`](p4kx_extraction::FilterError) of a Filter that is not accepted.
fn options(args: &GrepArgs) -> Result<SearchOptions> {
    let sources = if args.sources.is_empty() {
        Sources::ALL
    } else {
        args.sources.iter().copied().map(Source::from).collect()
    };
    Ok(SearchOptions {
        sources,
        filter: args.filter.as_deref().map(Filter::parse).transpose()?,
        exclusion: Exclusion::parse(&args.exclude)?,
        languages: args.lang.clone(),
        limit: args.limit as usize,
        per_item: args.per_item as usize,
        max_entry_len: args.max_entry_size,
    })
}

/// Writes what the search left out and how long it took to stderr.
fn notes(report: &SearchReport, elapsed: Duration, options: &SearchOptions) -> io::Result<()> {
    let mut err = io::stderr().lock();
    for source in report.has_more.iter() {
        writeln!(
            err,
            "note: more matches exist in {}; the first {} are shown, raise --limit",
            source.name(),
            options.limit
        )?;
    }
    for missing in &report.unavailable {
        writeln!(
            err,
            "note: {} not searched: {}",
            missing.source.name(),
            missing.reason
        )?;
    }
    let stats = report.stats;
    if stats.records_unreadable > 0 {
        writeln!(
            err,
            "note: {} Records could not be read and were skipped",
            stats.records_unreadable
        )?;
    }
    if stats.languages_unreadable > 0 {
        writeln!(
            err,
            "note: {} global.ini files could not be read and were skipped",
            stats.languages_unreadable
        )?;
    }
    skipped(&mut err, stats, options)?;
    writeln!(
        err,
        "searched {} Records, {} texts and {} Entries in {:.2} s: {} hits",
        stats.records_scanned,
        stats.texts_scanned,
        stats.entries_scanned,
        elapsed.as_secs_f64(),
        report.len()
    )
}

/// Writes how many Entries were not searched and why.
fn skipped(err: &mut impl Write, stats: SearchStats, options: &SearchOptions) -> io::Result<()> {
    if stats.entries_skipped() > 0 {
        writeln!(
            err,
            "note: {} Entries skipped: {} binary, {} larger than {} bytes, {} encrypted without \
             a matching Archive key, {} unreadable",
            stats.entries_skipped(),
            stats.entries_binary,
            stats.entries_too_large,
            options.max_entry_len,
            stats.entries_key_required,
            stats.entries_unreadable
        )?;
    }
    if stats.entries_key_required > 0 {
        writeln!(
            err,
            "hint: give the Archive key with --key <hex> or the P4KX_ARCHIVE_KEY environment \
             variable, or store it once with `p4kx key set`, to search encrypted Entries"
        )?;
    }
    Ok(())
}
