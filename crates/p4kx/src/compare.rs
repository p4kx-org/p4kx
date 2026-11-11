// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_comparison::{
    Archive, ArchiveError, CompareOptions, Comparison, DEFAULT_CONTEXT, DataForges,
    DirectoryCounts, Filter, Levels, Side, compare, line_diff,
};
use thiserror::Error;

use crate::cli::CompareArgs;
use crate::{compare_schema as schema, dataforges, diff, exit, json};

const UNKNOWN: &str = "unknown";
const ROOT: &str = "(root)";

/// The JSON report could not be written.
#[derive(Debug, Error)]
#[error("cannot write the report {}", path.display())]
pub(crate) struct ReportNotWritten {
    path: PathBuf,
    #[source]
    source: io::Error,
}

/// Compares both Archives, or prints the line diff of one Entry with `--diff`; writes the report
/// when asked, prints the summary or the report as JSON, and returns 0 without differences and 1
/// with differences, also when stdout was closed.
pub(crate) fn run(args: &CompareArgs, as_json: bool) -> Result<ExitCode> {
    let (old, new) = open_both(&args.old, &args.new)?;
    if let Some(path) = &args.diff {
        let diff = line_diff(&old, &new, path, DEFAULT_CONTEXT)?;
        return finish(
            diff::print(&diff, &old, &new, as_json),
            diff.has_differences(),
        );
    }
    if args.schema {
        return schema::run(&old, &new, as_json);
    }
    let filter = args.filter.as_deref().map(Filter::parse).transpose()?;
    let patches = (as_json || args.report.is_some())
        .then(crate::launcher_discovery::patches)
        .transpose()?;
    let options = CompareOptions {
        levels: levels(args),
        filter: filter.as_ref(),
        public_patches: patches.as_ref(),
        ..CompareOptions::default()
    };
    let forge_entries = if options.levels.records {
        Some(dataforges::read_both(&old, &new)?)
    } else {
        None
    };
    let forges = forge_entries
        .as_ref()
        .map(|(old, new)| dataforges::open_both(old, new))
        .transpose()?;
    let both = forges.as_ref().map(|(old, new)| DataForges { old, new });
    let comparison = compare(&old, &new, both, &options)?;
    if let Some(path) = &args.report {
        comparison
            .save_report(path)
            .map_err(|source| ReportNotWritten {
                path: path.clone(),
                source,
            })?;
    }
    let printed = if as_json {
        json::print(&comparison)
    } else {
        print_summary(&comparison)
    };
    finish(printed, comparison.has_differences())
}

/// Returns the exit code for a printed result, ignoring a closed stdout.
pub(crate) fn finish(printed: io::Result<()>, differs: bool) -> Result<ExitCode> {
    if let Err(error) = printed
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(error.into());
    }
    Ok(ExitCode::from(if differs {
        exit::DIFFERENCES
    } else {
        exit::SUCCESS
    }))
}

const fn levels(args: &CompareArgs) -> Levels {
    if args.entries || args.records {
        Levels::NONE
            .with_entries(args.entries)
            .with_records(args.records)
    } else {
        Levels::ALL
    }
}

fn open_both(old: &Path, new: &Path) -> Result<(Archive, Archive), ArchiveError> {
    std::thread::scope(|scope| {
        let new = scope.spawn(|| crate::key::open(new));
        let old = crate::key::open(old);
        let new = new
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        Ok((old?, new?))
    })
}

fn print_summary(comparison: &Comparison<'_>) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    write_side(&mut out, "Old", comparison.old_side())?;
    write_side(&mut out, "New", comparison.new_side())?;
    write_line(&mut out, "Filter", comparison.filter().unwrap_or("none"))?;
    if comparison.levels().entries {
        let counts = comparison.entry_counts();
        write_line(&mut out, "Added", &counts.added.to_string())?;
        write_line(&mut out, "Removed", &counts.removed.to_string())?;
        write_line(&mut out, "Changed", &counts.changed.to_string())?;
    }
    if comparison.levels().records {
        let counts = comparison.record_counts();
        write_line(&mut out, "Records added", &counts.added.to_string())?;
        write_line(&mut out, "Records removed", &counts.removed.to_string())?;
        write_line(&mut out, "Records moved", &counts.moved.to_string())?;
        write_line(&mut out, "Records changed", &counts.changed.to_string())?;
        write_line(
            &mut out,
            "Records unreadable",
            &counts.unreadable.to_string(),
        )?;
    }
    let directories = comparison.directory_counts();
    if !directories.is_empty() {
        writeln!(out)?;
        write_directory(&mut out, ["DIRECTORY", "ADDED", "REMOVED", "CHANGED"])?;
        for DirectoryCounts { directory, counts } in directories {
            let name = if directory.is_empty() {
                ROOT
            } else {
                directory
            };
            write_directory(
                &mut out,
                [
                    name,
                    &counts.added.to_string(),
                    &counts.removed.to_string(),
                    &counts.changed.to_string(),
                ],
            )?;
        }
    }
    out.flush()
}

pub(crate) fn write_side(out: &mut impl Write, label: &str, side: &Side<'_>) -> io::Result<()> {
    write_line(out, label, &json::path(side.archive()))?;
    write_line(
        out,
        &format!("{label} Channel"),
        side.channel().unwrap_or(UNKNOWN),
    )?;
    let build = side.build().map_or_else(
        || UNKNOWN.to_owned(),
        |build| {
            format!(
                "{} ({}, change {})",
                build.version(),
                build.branch(),
                build.change_number()
            )
        },
    );
    write_line(out, &format!("{label} Build"), &build)
}

pub(crate) fn write_line(out: &mut impl Write, label: &str, value: &str) -> io::Result<()> {
    writeln!(out, "{:<16} {value}", format!("{label}:"))
}

fn write_directory(
    out: &mut impl Write,
    [directory, added, removed, changed]: [&str; 4],
) -> io::Result<()> {
    writeln!(out, "{directory:<24} {added:>9} {removed:>9} {changed:>9}")
}
