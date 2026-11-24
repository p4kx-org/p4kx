// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_extraction::{CoverageReport, DataForgeEntry, ReferenceList, SampleExport};

use crate::cli::CoverageArgs;
use crate::cli_coverage::CoverageFormat;
use crate::dcb_refs::JsonWithTable;
use crate::{coverage_json, exit};

/// Checks the reference list against the Archive and writes the coverage to stdout; the exit code
/// is 1 when a listed type, component or folder is missing, unreadable or does not convert.
pub(crate) fn run(args: &CoverageArgs, as_json: bool) -> Result<ExitCode> {
    let json = match (as_json, args.format) {
        (true, Some(CoverageFormat::Table)) => return Err(JsonWithTable.into()),
        (as_json, format) => as_json || format == Some(CoverageFormat::Json),
    };
    let archive = crate::key::open(&args.archive)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let references = ReferenceList::builtin()?;
    let (written, complete) = if json {
        let observed = CoverageReport::observed(&archive, &forge, &entry, &references)?;
        (
            coverage_json::print(&args.archive, &observed),
            observed.report().is_complete(),
        )
    } else {
        let report = CoverageReport::of(&archive, &forge, &references);
        (table(&report), report.is_complete())
    };
    if let Err(error) = written
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(error.into());
    }
    Ok(ExitCode::from(if complete {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }))
}

fn sample_note(sample: &SampleExport) -> String {
    sample.error.as_ref().map_or_else(
        || {
            format!(
                "sample {} B XML, {} B JSON",
                sample.xml_bytes, sample.json_bytes
            )
        },
        |error| format!("sample failed: {error}"),
    )
}

fn table(report: &CoverageReport) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    types(&mut out, report)?;
    components(&mut out, report)?;
    objects(&mut out, report)?;
    writeln!(out, "Entry folders")?;
    for found in &report.entries {
        writeln!(
            out,
            "  {:<12} {}: {} Entries, {} converted, {} text, {} failed",
            found.area,
            found.folder,
            found.entries,
            found.converted,
            found.text,
            found.failures.len(),
        )?;
        for (path, reason) in &found.failures {
            writeln!(out, "    {path}: {reason}")?;
        }
    }
    out.flush()
}

fn types(out: &mut impl Write, report: &CoverageReport) -> io::Result<()> {
    writeln!(out, "Record types")?;
    for found in &report.types {
        let note = match (&found.sample, found.defined) {
            (_, false) => "not defined".to_owned(),
            (None, true) => "no Records".to_owned(),
            (Some(sample), true) => sample_note(sample),
        };
        let candidate = if found.candidate { " (candidate)" } else { "" };
        writeln!(
            out,
            "  {:<12} {:<34} {:>8} Records, {} unreadable, {} error values; {note}{candidate}",
            found.area, found.name, found.records, found.unreadable_records, found.error_values,
        )?;
    }
    Ok(())
}

fn components(out: &mut impl Write, report: &CoverageReport) -> io::Result<()> {
    writeln!(
        out,
        "Components of EntityClassDefinition ({} Records unreadable)",
        report.unreadable_entities
    )?;
    for found in &report.components {
        let state = match (found.defined, found.reachable) {
            (false, _) => "not defined",
            (true, false) => "not reachable by a Query",
            (true, true) => "reachable",
        };
        writeln!(
            out,
            "  {:<12} {:<44} {:>8} entities, {:>8} instances; {state}",
            found.area, found.name, found.entities, found.instances,
        )?;
    }
    Ok(())
}

fn objects(out: &mut impl Write, report: &CoverageReport) -> io::Result<()> {
    writeln!(out, "StarMapObject Records by StarMapObjectType")?;
    for found in &report.objects {
        let note = found
            .sample
            .as_ref()
            .map_or_else(|| "no Records".to_owned(), sample_note);
        writeln!(
            out,
            "  {:<12} {:<34} {:>8} Records, {} templates; {note}",
            found.area, found.type_name, found.records, found.templates,
        )?;
    }
    Ok(())
}
