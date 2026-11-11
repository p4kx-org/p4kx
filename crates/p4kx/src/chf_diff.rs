// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_assembly::character::{CHF_SIZE, ChfChange, NameTable, diff_chf, diff_chf_observed};
use serde::Serialize;

use crate::chf::{read_capped, read_capped_observed, read_chf};
use crate::chf_diff_origins::{DiffFieldOrigins, DiffOrigins};
use crate::chf_names;
use crate::cli::{ChfDiffArgs, ChfDiffFormat};
use crate::compare::finish;
use crate::json;

#[derive(Serialize)]
struct Report<'a> {
    a: String,
    b: String,
    identical: bool,
    changes: Vec<ChangeJson<'a>>,
    #[serde(flatten)]
    origins: DiffFieldOrigins,
}

#[derive(Serialize)]
struct ChangeJson<'a> {
    path: &'a str,
    raw_path: &'a str,
    old: Option<&'a str>,
    new: Option<&'a str>,
}

/// Runs `p4kx chf diff`.
pub(crate) fn run(args: &ChfDiffArgs, as_json: bool) -> Result<ExitCode> {
    if as_json || args.format == ChfDiffFormat::Json {
        return report(args);
    }
    let old = read_chf(&args.a, &read_capped(&args.a, CHF_SIZE as u64)?)?;
    let new = read_chf(&args.b, &read_capped(&args.b, CHF_SIZE as u64)?)?;
    let names = chf_names::names_for_all(args.archive.as_deref(), &[&old, &new])?;
    let changes = diff_chf(&old, &new, &names);
    finish(print_text(&changes), !changes.is_empty())
}

fn report(args: &ChfDiffArgs) -> Result<ExitCode> {
    let before = read_capped_observed(&args.a, CHF_SIZE as u64)?;
    let old = read_chf(&args.a, before.bytes())?;
    let after = read_capped_observed(&args.b, CHF_SIZE as u64)?;
    let new = read_chf(&args.b, after.bytes())?;
    let origins = DiffOrigins::new(&before, &after)?;
    let names = args
        .archive
        .as_deref()
        .map(|path| chf_names::observed_all(path, &[&old, &new], origins.uses()))
        .transpose()?;
    let builtin = NameTable::builtin();
    let table = names
        .as_ref()
        .map_or(&builtin, chf_names::ObservedNames::table);
    let changes = diff_chf_observed(&old, &new, table);
    let report = Report {
        a: json::path(&args.a),
        b: json::path(&args.b),
        identical: changes.is_empty(),
        changes: changes
            .iter()
            .map(|observed| {
                let change = observed.change();
                ChangeJson {
                    path: &change.path,
                    raw_path: &change.raw_path,
                    old: change.old.as_deref(),
                    new: change.new.as_deref(),
                }
            })
            .collect(),
        origins: origins.project(&changes, names.as_ref())?,
    };
    finish(json::print(&report), !report.identical)
}

fn print_text(changes: &[ChfChange]) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    if changes.is_empty() {
        writeln!(out, "no differences")?;
    }
    for change in changes {
        match (&change.old, &change.new) {
            (Some(old), Some(new)) => writeln!(out, "{}: {old} -> {new}", change.path)?,
            (Some(old), None) => writeln!(out, "{}: removed ({old})", change.path)?,
            (None, Some(new)) => writeln!(out, "{}: added ({new})", change.path)?,
            (None, None) => {}
        }
    }
    out.flush()
}
