// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use p4kx_export::{ActionMapCounts, read_action_maps, write_action_maps};
use p4kx_extraction::{Archive, LocalizationSet};
use serde::Serialize;

use crate::cli_actionmap::{ActionMapAction, ActionMapArgs, ActionMapExportArgs};
use crate::json;
use crate::loc::{LanguageNotInArchive, print_notes};

#[derive(Serialize)]
struct ExportJson<'a> {
    archive: String,
    out: String,
    format: &'static str,
    language: &'a str,
    action_maps: usize,
    actions: usize,
    unresolved_labels: usize,
}

/// Runs `p4kx actionmap`.
pub(crate) fn run(args: &ActionMapArgs, as_json: bool) -> Result<()> {
    match &args.action {
        ActionMapAction::Export(args) => export(args, as_json),
    }
}

fn export(args: &ActionMapExportArgs, as_json: bool) -> Result<()> {
    let format = args.library_format()?;
    let archive = crate::key::open(&args.archive)?;
    crate::guard::ensure_writable_if(args.out.as_deref())?;
    let maps = read_action_maps(&archive)?;
    let set = LocalizationSet::read(&archive, &[args.language])?;
    if set.entry(args.language).is_none() {
        return Err(LanguageNotInArchive {
            language: args.language,
        }
        .into());
    }
    let texts = set.index()?;
    print_notes(&set.notes(&texts));
    let Some(path) = &args.out else {
        let mut out = BufWriter::new(io::stdout().lock());
        write_action_maps(&maps, &texts, args.language, format, &mut out)?;
        out.flush()?;
        return Ok(());
    };
    let cancel = p4kx_extraction::CancelToken::new();
    let protection = p4kx_extraction::ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let counts = crate::query_file::write_replacing_protected(
        path,
        Some(&protection),
        Some(&cancel),
        |out| write_action_maps(&maps, &texts, args.language, format, out),
    )
    .with_context(|| format!("cannot write {}", path.display()))?;
    report(args, path, as_json, &archive, &counts)
}

fn report(
    args: &ActionMapExportArgs,
    path: &Path,
    as_json: bool,
    archive: &Archive,
    counts: &ActionMapCounts,
) -> Result<()> {
    if as_json {
        json::print(&ExportJson {
            archive: json::path(archive.path()),
            out: json::path(path),
            format: args.format.name(),
            language: args.language.code(),
            action_maps: counts.action_maps,
            actions: counts.actions,
            unresolved_labels: counts.unresolved_labels,
        })?;
    } else {
        writeln!(
            io::stdout().lock(),
            "wrote {}: {} actions in {} action maps, unresolved labels in {}: {}",
            path.display(),
            counts.actions,
            counts.action_maps,
            args.language,
            counts.unresolved_labels,
        )?;
    }
    Ok(())
}
