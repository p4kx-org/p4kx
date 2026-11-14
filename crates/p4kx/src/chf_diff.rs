// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_assembly::character::{
    CHF_SIZE, ChfObservedChange, NameTable, diff_chf, diff_chf_observed,
};
use serde::Serialize;
use thiserror::Error;

use crate::chf::{read_capped, read_capped_observed, read_chf};
use crate::chf_diff_origins::{DiffFieldOrigins, DiffOrigins, Projection};
use crate::chf_names;
use crate::cli::{ChfDiffArgs, ChfDiffFormat};
use crate::compare::finish;
use crate::json;
use crate::{chf_diff_text, chf_diff_text_output};

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

#[derive(Debug, Error)]
#[error("--origin-stderr and --origin-out require text comparison output")]
pub(crate) struct TextOriginsNeedText;

/// Runs `p4kx chf diff`.
pub(crate) fn run(args: &ChfDiffArgs, as_json: bool) -> Result<ExitCode> {
    let with_origin = args.origin_stderr || args.origin_out.is_some();
    if as_json || args.format == ChfDiffFormat::Json {
        if with_origin {
            return Err(TextOriginsNeedText.into());
        }
        return report(args, observed_comparison(args, Projection::Json)?);
    }
    if with_origin {
        let comparison = observed_comparison(args, Projection::Text)?;
        chf_diff_text_output::write(args, &comparison.changes, &comparison.origins)?;
        return finish(Ok(()), !comparison.changes.is_empty());
    }
    let old = read_chf(&args.a, &read_capped(&args.a, CHF_SIZE as u64)?)?;
    let new = read_chf(&args.b, &read_capped(&args.b, CHF_SIZE as u64)?)?;
    let names = chf_names::names_for_all(args.archive.as_deref(), &[&old, &new])?;
    let changes = diff_chf(&old, &new, &names);
    let mut out = BufWriter::new(io::stdout().lock());
    finish(
        chf_diff_text::write(&mut out, &changes, |_| Ok(())),
        !changes.is_empty(),
    )
}

struct ObservedComparison {
    changes: Vec<ChfObservedChange>,
    origins: DiffFieldOrigins,
}

fn observed_comparison(args: &ChfDiffArgs, projection: Projection) -> Result<ObservedComparison> {
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
    let origins = origins.project(&changes, names.as_ref(), projection)?;
    Ok(ObservedComparison { changes, origins })
}

fn report(args: &ChfDiffArgs, comparison: ObservedComparison) -> Result<ExitCode> {
    let ObservedComparison { changes, origins } = comparison;
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
        origins,
    };
    finish(json::print(&report), !report.identical)
}
